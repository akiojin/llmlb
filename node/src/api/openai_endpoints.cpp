#include "api/openai_endpoints.h"

#include <nlohmann/json.hpp>
#include <limits>
#include <memory>
#include <cctype>
#include <algorithm>
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "runtime/state.h"
#include "utils/utf8.h"

namespace llm_node {

using json = nlohmann::json;

namespace {
// SPEC-dcaeaec4: Helper to check if node is ready and return 503 if not
bool checkReady(httplib::Response& res) {
    if (!is_ready()) {
        res.status = 503;
        nlohmann::json err = {
            {"error", {
                {"type", "service_unavailable"},
                {"message", "Node is syncing models with router. Please wait."}
            }}
        };
        res.set_content(err.dump(), "application/json");
        return false;
    }
    return true;
}
bool isBlank(const std::string& value) {
    return std::all_of(value.begin(), value.end(),
        [](unsigned char c) { return std::isspace(c) != 0; });
}

bool parseInferenceParams(const nlohmann::json& body, InferenceParams& params, std::string& error) {
    params = InferenceParams{};

    // OpenAI-compatible fields
    if (body.contains("max_tokens") && body["max_tokens"].is_number_integer()) {
        int v = body["max_tokens"].get<int>();
        if (v > 0) params.max_tokens = static_cast<size_t>(v);
    }
    if (body.contains("temperature")) {
        if (!body["temperature"].is_number()) {
            error = "temperature must be a number";
            return false;
        }
        const float value = body["temperature"].get<float>();
        if (value < 0.0f || value > 2.0f) {
            error = "temperature must be between 0 and 2";
            return false;
        }
        params.temperature = value;
    }
    if (body.contains("top_p")) {
        if (!body["top_p"].is_number()) {
            error = "top_p must be a number";
            return false;
        }
        const float value = body["top_p"].get<float>();
        if (value < 0.0f || value > 1.0f) {
            error = "top_p must be between 0 and 1";
            return false;
        }
        params.top_p = value;
    }
    if (body.contains("top_k")) {
        if (!body["top_k"].is_number_integer()) {
            error = "top_k must be an integer";
            return false;
        }
        const int value = body["top_k"].get<int>();
        if (value < 0) {
            error = "top_k must be >= 0";
            return false;
        }
        params.top_k = value;
    }
    if (body.contains("repeat_penalty") && body["repeat_penalty"].is_number()) {
        params.repeat_penalty = body["repeat_penalty"].get<float>();
    }
    if (body.contains("seed") && body["seed"].is_number_integer()) {
        int64_t v = body["seed"].get<int64_t>();
        if (v > 0 && v <= static_cast<int64_t>(std::numeric_limits<uint32_t>::max())) {
            params.seed = static_cast<uint32_t>(v);
        }
    }

    return true;
}

bool parseStopSequences(const nlohmann::json& body, std::vector<std::string>& stops, std::string& error) {
    stops.clear();
    if (!body.contains("stop") || body["stop"].is_null()) {
        return true;
    }
    const auto& stop = body["stop"];
    if (stop.is_string()) {
        auto value = stop.get<std::string>();
        if (!value.empty()) stops.push_back(std::move(value));
        return true;
    }
    if (stop.is_array()) {
        for (const auto& item : stop) {
            if (!item.is_string()) {
                error = "stop must be a string or array of strings";
                return false;
            }
            auto value = item.get<std::string>();
            if (!value.empty()) stops.push_back(std::move(value));
        }
        return true;
    }
    error = "stop must be a string or array of strings";
    return false;
}

struct LogprobsOptions {
    bool enabled{false};
    int top_logprobs{0};
};

bool parseLogprobsOptions(const nlohmann::json& body, LogprobsOptions& options, std::string& error) {
    options = LogprobsOptions{};
    if (!body.contains("logprobs") || body["logprobs"].is_null()) {
        if (body.contains("top_logprobs") && !body["top_logprobs"].is_null()) {
            error = "top_logprobs requires logprobs to be true";
            return false;
        }
        return true;
    }

    const auto& logprobs = body["logprobs"];
    if (logprobs.is_boolean()) {
        options.enabled = logprobs.get<bool>();
    } else if (logprobs.is_number_integer()) {
        const int value = logprobs.get<int>();
        if (value < 0) {
            error = "logprobs must be >= 0";
            return false;
        }
        options.enabled = value > 0;
        options.top_logprobs = value;
    } else {
        error = "logprobs must be a boolean or integer";
        return false;
    }

    if (body.contains("top_logprobs") && !body["top_logprobs"].is_null()) {
        if (!options.enabled) {
            error = "top_logprobs requires logprobs to be true";
            return false;
        }
        if (!body["top_logprobs"].is_number_integer()) {
            error = "top_logprobs must be an integer";
            return false;
        }
        const int value = body["top_logprobs"].get<int>();
        if (value < 0) {
            error = "top_logprobs must be >= 0";
            return false;
        }
        options.top_logprobs = value;
    }

    return true;
}

std::vector<std::string> splitLogprobsTokens(const std::string& text) {
    std::vector<std::string> tokens;
    std::string current;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty()) {
        tokens.push_back(current);
    }
    return tokens;
}

json buildLogprobs(const std::string& output, int top_logprobs) {
    json tokens_json = json::array();
    json token_logprobs = json::array();
    json top_logprobs_json = json::array();

    const auto tokens = splitLogprobsTokens(output);
    for (const auto& token : tokens) {
        tokens_json.push_back(token);
        token_logprobs.push_back(0.0);

        json top = json::object();
        if (top_logprobs > 0) {
            top[token] = 0.0;
        }
        top_logprobs_json.push_back(top);
    }

    return json{
        {"tokens", tokens_json},
        {"token_logprobs", token_logprobs},
        {"top_logprobs", top_logprobs_json}
    };
}

std::string applyStopSequences(std::string output, const std::vector<std::string>& stops) {
    if (stops.empty()) return output;
    size_t earliest = std::string::npos;
    for (const auto& stop : stops) {
        if (stop.empty()) continue;
        size_t pos = output.find(stop);
        if (pos != std::string::npos && (earliest == std::string::npos || pos < earliest)) {
            earliest = pos;
        }
    }
    if (earliest == std::string::npos) return output;
    output.resize(earliest);
    return output;
}
}  // namespace

struct ParsedChatMessages {
    std::vector<ChatMessage> messages;
    std::vector<std::string> image_urls;
};

constexpr size_t kMaxImageCount = 10;
const std::string kVisionMarker = "<__media__>";

bool parseChatMessages(const json& body, ParsedChatMessages& out, std::string& error) {
    out.messages.clear();
    out.image_urls.clear();

    if (!body.contains("messages")) {
        return true;
    }
    if (!body["messages"].is_array()) {
        error = "messages must be an array";
        return false;
    }

    for (const auto& m : body["messages"]) {
        if (!m.is_object()) {
            error = "message must be an object";
            return false;
        }
        std::string role = m.value("role", "");
        if (role.empty()) {
            error = "message.role is required";
            return false;
        }

        std::string content;
        if (!m.contains("content") || m["content"].is_null()) {
            out.messages.push_back({role, ""});
            continue;
        }

        const auto& c = m["content"];
        if (c.is_string()) {
            content = c.get<std::string>();
        } else if (c.is_array()) {
            for (const auto& part : c) {
                if (!part.is_object()) {
                    error = "content part must be an object";
                    return false;
                }
                std::string type = part.value("type", "");
                if (type == "text") {
                    content += part.value("text", "");
                } else if (type == "image_url") {
                    std::string url;
                    if (part.contains("image_url")) {
                        const auto& image_url = part["image_url"];
                        if (image_url.is_object()) {
                            url = image_url.value("url", "");
                        } else if (image_url.is_string()) {
                            url = image_url.get<std::string>();
                        }
                    }
                    if (url.empty()) {
                        error = "image_url.url is required";
                        return false;
                    }
                    out.image_urls.push_back(url);
                    if (out.image_urls.size() > kMaxImageCount) {
                        error = "too many images in request";
                        return false;
                    }
                    content += kVisionMarker;
                } else {
                    error = "unsupported content type: " + type;
                    return false;
                }
            }
        } else {
            error = "content must be a string or array";
            return false;
        }

        out.messages.push_back({role, content});
    }

    return true;
}

OpenAIEndpoints::OpenAIEndpoints(ModelRegistry& registry, InferenceEngine& engine, const NodeConfig& config)
    : registry_(registry), engine_(engine), config_(config) {}

void OpenAIEndpoints::registerRoutes(httplib::Server& server) {
    server.Get("/v1/models", [this](const httplib::Request&, httplib::Response& res) {
        json body;
        body["object"] = "list";
        body["data"] = json::array();
        for (const auto& id : registry_.listModels()) {
            body["data"].push_back({{"id", id}, {"object", "model"}});
        }
        setJson(res, body);
    });

    server.Post("/v1/chat/completions", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::try_acquire();
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;
            ParsedChatMessages parsed;
            std::string parse_error;
            if (!parseChatMessages(body, parsed, parse_error)) {
                respondError(res, 400, "bad_request", parse_error);
                return;
            }
            bool stream = body.value("stream", false);
            InferenceParams params;
            std::string params_error;
            if (!parseInferenceParams(body, params, params_error)) {
                respondError(res, 400, "invalid_request", params_error);
                return;
            }
            std::vector<std::string> stops;
            std::string stop_error;
            if (!parseStopSequences(body, stops, stop_error)) {
                respondError(res, 400, "invalid_request", stop_error);
                return;
            }
            std::string output;
            if (!parsed.image_urls.empty()) {
                output = engine_.generateChatWithImages(parsed.messages, parsed.image_urls, model, params);
            } else {
                output = engine_.generateChat(parsed.messages, model, params);
            }
            output = applyStopSequences(std::move(output), stops);
            output = sanitize_utf8_lossy(output);

            if (stream) {
                auto guard_ptr = std::make_shared<RequestGuard>(std::move(*guard));
                res.set_header("Content-Type", "text/event-stream");
                res.set_chunked_content_provider("text/event-stream",
                    [output, guard_ptr](size_t offset, httplib::DataSink& sink) {
                        if (offset == 0) {
                            // OpenAI compatible streaming format
                            json event_data = {
                                {"id", "chatcmpl-1"},
                                {"object", "chat.completion.chunk"},
                                {"choices", json::array({{
                                    {"index", 0},
                                    {"delta", {{"content", output}}},
                                    {"finish_reason", nullptr}
                                }})}
                            };
                            std::string chunk = "data: " + event_data.dump() + "\n\n";
                            sink.write(chunk.data(), chunk.size());
                            std::string done = "data: [DONE]\n\n";
                            sink.write(done.data(), done.size());
                            sink.done();
                        }
                        return true;
                    });
                return;
            }

            json resp = {
                {"id", "chatcmpl-1"},
                {"object", "chat.completion"},
                {"choices", json::array({{
                    {"index", 0},
                    {"message", {{"role", "assistant"}, {"content", output}}},
                    {"finish_reason", "stop"}
                }})}
            };
            setJson(res, resp);
        } catch (const std::exception& e) {
            respondError(res, 400, "bad_request", std::string("error: ") + e.what());
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/completions", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::try_acquire();
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;
            std::string prompt = body.value("prompt", "");
            if (prompt.empty() || isBlank(prompt)) {
                respondError(res, 400, "invalid_request", "prompt must not be empty");
                return;
            }
            InferenceParams params;
            std::string params_error;
            if (!parseInferenceParams(body, params, params_error)) {
                respondError(res, 400, "invalid_request", params_error);
                return;
            }
            std::vector<std::string> stops;
            std::string stop_error;
            if (!parseStopSequences(body, stops, stop_error)) {
                respondError(res, 400, "invalid_request", stop_error);
                return;
            }
            LogprobsOptions logprobs_options;
            std::string logprobs_error;
            if (!parseLogprobsOptions(body, logprobs_options, logprobs_error)) {
                respondError(res, 400, "invalid_request", logprobs_error);
                return;
            }
            std::string output = engine_.generateCompletion(prompt, model, params);
            output = applyStopSequences(std::move(output), stops);
            output = sanitize_utf8_lossy(output);
            json choice = {
                {"text", output},
                {"index", 0},
                {"finish_reason", "stop"}
            };
            if (logprobs_options.enabled) {
                choice["logprobs"] = buildLogprobs(output, logprobs_options.top_logprobs);
            }
            json resp = {
                {"id", "cmpl-1"},
                {"object", "text_completion"},
                {"choices", json::array({choice})}
            };
            setJson(res, resp);
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/embeddings", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::try_acquire();
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            // モデルパラメータは必須（OpenAI API仕様準拠）
            if (!body.contains("model") || !body["model"].is_string() || body["model"].get<std::string>().empty()) {
                respondError(res, 400, "invalid_request", "model is required");
                return;
            }
            std::string model = body["model"].get<std::string>();
            if (!validateModel(model, "embeddings", res)) return;

            // inputを解析（文字列または文字列の配列）
            std::vector<std::string> inputs;
            if (body.contains("input")) {
                if (body["input"].is_string()) {
                    inputs.push_back(body["input"].get<std::string>());
                } else if (body["input"].is_array()) {
                    for (const auto& item : body["input"]) {
                        if (item.is_string()) {
                            inputs.push_back(item.get<std::string>());
                        }
                    }
                }
            }

            if (inputs.empty()) {
                respondError(res, 400, "invalid_request", "input is required");
                return;
            }

            // embeddingを生成
            auto embeddings = engine_.generateEmbeddings(inputs, model);

            // OpenAI互換レスポンスを構築
            json data = json::array();
            int total_tokens = 0;
            for (size_t i = 0; i < embeddings.size(); ++i) {
                data.push_back({
                    {"object", "embedding"},
                    {"embedding", embeddings[i]},
                    {"index", static_cast<int>(i)}
                });
                // トークン数の概算（文字数 / 4）
                total_tokens += static_cast<int>(inputs[i].size() / 4 + 1);
            }

            json resp = {
                {"object", "list"},
                {"data", data},
                {"model", model},
                {"usage", {{"prompt_tokens", total_tokens}, {"total_tokens", total_tokens}}}
            };
            setJson(res, resp);
        } catch (const std::exception& e) {
            respondError(res, 500, "internal_error", std::string("embedding error: ") + e.what());
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });
}

void OpenAIEndpoints::setJson(httplib::Response& res, const nlohmann::json& body) {
    res.set_content(body.dump(), "application/json");
}

void OpenAIEndpoints::respondError(httplib::Response& res, int status, const std::string& code, const std::string& message) {
    res.status = status;
    setJson(res, {{"error", code}, {"message", message}});
}

bool OpenAIEndpoints::validateModel(const std::string& model,
                                    const std::string& capability,
                                    httplib::Response& res) {
    if (model.empty()) {
        respondError(res, 400, "model_required", "model is required");
        return false;
    }
    // Check local registry first
    const bool in_registry = registry_.hasModel(model);
    if (in_registry && !engine_.isInitialized()) {
        return true;
    }
    // Try to resolve/load via ModelResolver (local -> shared -> router API)
    // loadModel() handles the full resolution flow
    auto load_result = engine_.loadModel(model, capability);
    if (!load_result.success) {
        const std::string prefix = "Model does not support capability:";
        if (load_result.error_message.rfind(prefix, 0) == 0) {
            respondError(res, 400, "invalid_request", load_result.error_message);
            return false;
        }
        respondError(res, 404, "model_not_found",
            load_result.error_message.empty() ? "model not found" : load_result.error_message);
        return false;
    }
    return true;
}

}  // namespace llm_node
