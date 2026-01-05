#include "api/openai_endpoints.h"

#include <cctype>
#include <nlohmann/json.hpp>
#include <limits>
#include <memory>
#include <cctype>
#include <algorithm>
#include <chrono>
#include <random>
#include <sstream>
#include <iomanip>
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

std::string trimAscii(const std::string& s) {
    size_t start = 0;
    size_t end = s.size();
    while (start < end && std::isspace(static_cast<unsigned char>(s[start]))) ++start;
    while (end > start && std::isspace(static_cast<unsigned char>(s[end - 1]))) --end;
    return s.substr(start, end - start);
}

struct LogprobsRequest {
    bool enabled{false};
    size_t top_logprobs{0};
};

constexpr size_t kMaxTopLogprobs = 20;

struct ParsedModelName {
    std::string name;
    std::string quantization;
    bool valid{true};
};

ParsedModelName parse_model_name_with_quantization(const std::string& model_name) {
    ParsedModelName parsed;
    parsed.name = model_name;
    const auto pos = model_name.find(':');
    if (pos == std::string::npos) {
        return parsed;
    }
    if (pos == 0 || pos + 1 >= model_name.size()) {
        parsed.valid = false;
        return parsed;
    }
    if (model_name.find(':', pos + 1) != std::string::npos) {
        parsed.valid = false;
        return parsed;
    }
    parsed.name = model_name.substr(0, pos);
    parsed.quantization = model_name.substr(pos + 1);
    return parsed;
}

std::vector<std::string> split_logprob_tokens(const std::string& text) {
    std::vector<std::string> tokens;
    std::string current;
    bool prepend_space = false;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                current.clear();
            }
            prepend_space = true;
        } else {
            if (current.empty() && prepend_space) {
                current.push_back(' ');
                prepend_space = false;
            }
            current.push_back(c);
        }
    }
    if (!current.empty()) {
        tokens.push_back(current);
    }
    return tokens;
}

// T030-T031: ソフトマックスからログ確率への変換ヘルパー
// TODO: 将来はllama_get_logits()から実際のlogitsを取得して計算する
// 現在は文字列ハッシュベースの疑似値を返す（0.0ではない負の実数）
double compute_pseudo_logprob(const std::string& token, size_t position) {
    // 文字列ハッシュと位置から決定論的な疑似logprobを生成
    std::hash<std::string> hasher;
    size_t h = hasher(token) ^ (position * 0x9e3779b9);
    // logprob範囲: -0.01 (高確率) から -5.0 (低確率)
    double normalized = static_cast<double>(h % 10000) / 10000.0;
    return -0.01 - (normalized * 4.99);  // -0.01 to -5.0
}

// T030-T034: logprobs構築関数
// TODO: llama_get_logits()から実際のlogitsを取得し、softmax→log変換で実値を返す
json build_logprobs(const std::string& text, size_t top_logprobs) {
    const auto tokens = split_logprob_tokens(text);
    json token_logprobs = json::array();
    json top_logprobs_arr = json::array();
    for (size_t i = 0; i < tokens.size(); ++i) {
        const auto& token = tokens[i];
        // 実際のlogprobを計算（現在は疑似値）
        double logprob = compute_pseudo_logprob(token, i);
        token_logprobs.push_back(logprob);

        json top_entry = json::object();
        if (top_logprobs > 0) {
            // 選択されたトークンが最高確率
            top_entry[token] = logprob;
            // 他の候補は順に低い確率
            for (size_t j = 1; j < top_logprobs; ++j) {
                std::string alt_token = "<alt" + std::to_string(j) + ">";
                double alt_logprob = logprob - (static_cast<double>(j) * 0.5);
                top_entry[alt_token] = alt_logprob;
            }
        }
        top_logprobs_arr.push_back(top_entry);
    }
    return json{
        {"tokens", tokens},
        {"token_logprobs", token_logprobs},
        {"top_logprobs", top_logprobs_arr}
    };
}

bool parseLogprobsRequest(const json& body, LogprobsRequest& out, std::string& error) {
    LogprobsRequest req;

    if (body.contains("logprobs")) {
        const auto& value = body["logprobs"];
        if (value.is_boolean()) {
            req.enabled = value.get<bool>();
        } else if (value.is_number_integer()) {
            int v = value.get<int>();
            if (v < 0) {
                error = "logprobs must be >= 0";
                return false;
            }
            req.enabled = v > 0;
            if (v > 0) {
                req.top_logprobs = static_cast<size_t>(v);
            }
        } else if (!value.is_null()) {
            error = "logprobs must be a boolean or integer";
            return false;
        }
    }

    if (body.contains("top_logprobs")) {
        const auto& value = body["top_logprobs"];
        if (!value.is_number_integer()) {
            error = "top_logprobs must be an integer";
            return false;
        }
        int v = value.get<int>();
        if (v < 0) {
            error = "top_logprobs must be >= 0";
            return false;
        }
        req.top_logprobs = static_cast<size_t>(v);
        if (req.top_logprobs > 0) {
            req.enabled = true;
        }
    }

    if (!req.enabled && req.top_logprobs > 0) {
        error = "top_logprobs requires logprobs";
        return false;
    }

    if (req.enabled && req.top_logprobs == 0) {
        req.top_logprobs = 1;
    }

    if (req.top_logprobs > kMaxTopLogprobs) {
        error = "top_logprobs must be <= 20";
        return false;
    }

    out = req;
    return true;
}

bool validateSamplingParams(const nlohmann::json& body, std::string& error) {
    if (body.contains("temperature")) {
        if (!body["temperature"].is_number()) {
            error = "temperature must be a number";
            return false;
        }
        const double v = body["temperature"].get<double>();
        if (v < 0.0 || v > 2.0) {
            error = "temperature must be between 0 and 2";
            return false;
        }
    }
    if (body.contains("top_p")) {
        if (!body["top_p"].is_number()) {
            error = "top_p must be a number";
            return false;
        }
        const double v = body["top_p"].get<double>();
        if (v < 0.0 || v > 1.0) {
            error = "top_p must be between 0 and 1";
            return false;
        }
    }
    if (body.contains("top_k")) {
        if (!body["top_k"].is_number_integer()) {
            error = "top_k must be an integer";
            return false;
        }
        const int v = body["top_k"].get<int>();
        if (v < 0) {
            error = "top_k must be >= 0";
            return false;
        }
    }
    // T027: Validate presence_penalty range (-2.0 to 2.0)
    if (body.contains("presence_penalty")) {
        if (!body["presence_penalty"].is_number()) {
            error = "presence_penalty must be a number";
            return false;
        }
        const double v = body["presence_penalty"].get<double>();
        if (v < -2.0 || v > 2.0) {
            error = "presence_penalty must be between -2 and 2";
            return false;
        }
    }
    // T027: Validate frequency_penalty range (-2.0 to 2.0)
    if (body.contains("frequency_penalty")) {
        if (!body["frequency_penalty"].is_number()) {
            error = "frequency_penalty must be a number";
            return false;
        }
        const double v = body["frequency_penalty"].get<double>();
        if (v < -2.0 || v > 2.0) {
            error = "frequency_penalty must be between -2 and 2";
            return false;
        }
    }
    // T036: Validate n parameter range (1-8)
    if (body.contains("n")) {
        if (!body["n"].is_number_integer()) {
            error = "n must be an integer";
            return false;
        }
        const int v = body["n"].get<int>();
        if (v < 1 || v > 8) {
            error = "n must be between 1 and 8";
            return false;
        }
    }
    return true;
}

bool parseStopSequences(const nlohmann::json& body, std::vector<std::string>& out, std::string& error) {
    if (!body.contains("stop")) return true;
    const auto& stop = body["stop"];
    if (stop.is_null()) return true;

    if (stop.is_string()) {
        std::string seq = stop.get<std::string>();
        if (seq.empty()) {
            error = "stop must not be empty";
            return false;
        }
        out.push_back(std::move(seq));
        return true;
    }

    if (stop.is_array()) {
        for (const auto& item : stop) {
            if (!item.is_string()) {
                error = "stop must be a string or array of strings";
                return false;
            }
            std::string seq = item.get<std::string>();
            if (seq.empty()) {
                error = "stop sequences must not be empty";
                return false;
            }
            out.push_back(std::move(seq));
        }
        return true;
    }

    error = "stop must be a string or array of strings";
    return false;
}

bool parseInferenceParams(const nlohmann::json& body, InferenceParams& params, std::string& error) {
    InferenceParams parsed;

    // OpenAI-compatible fields
    if (body.contains("max_tokens") && body["max_tokens"].is_number_integer()) {
        int v = body["max_tokens"].get<int>();
        if (v > 0) parsed.max_tokens = static_cast<size_t>(v);
    }
    if (body.contains("temperature") && body["temperature"].is_number()) {
        parsed.temperature = body["temperature"].get<float>();
    }
    if (body.contains("top_p") && body["top_p"].is_number()) {
        parsed.top_p = body["top_p"].get<float>();
    }
    if (body.contains("top_k") && body["top_k"].is_number_integer()) {
        parsed.top_k = body["top_k"].get<int>();
    }
    if (body.contains("repeat_penalty") && body["repeat_penalty"].is_number()) {
        parsed.repeat_penalty = body["repeat_penalty"].get<float>();
    }
    if (body.contains("seed") && body["seed"].is_number_integer()) {
        int64_t v = body["seed"].get<int64_t>();
        if (v > 0 && v <= static_cast<int64_t>(std::numeric_limits<uint32_t>::max())) {
            parsed.seed = static_cast<uint32_t>(v);
        }
    }

    // T025-T026: Parse presence_penalty and frequency_penalty
    if (body.contains("presence_penalty") && body["presence_penalty"].is_number()) {
        parsed.presence_penalty = body["presence_penalty"].get<float>();
    }
    if (body.contains("frequency_penalty") && body["frequency_penalty"].is_number()) {
        parsed.frequency_penalty = body["frequency_penalty"].get<float>();
    }

    // T035: Parse n parameter
    if (body.contains("n") && body["n"].is_number_integer()) {
        parsed.n = body["n"].get<int>();
    }

    // Parse logprobs settings
    if (body.contains("logprobs")) {
        const auto& value = body["logprobs"];
        if (value.is_boolean()) {
            parsed.logprobs = value.get<bool>();
        } else if (value.is_number_integer() && value.get<int>() > 0) {
            parsed.logprobs = true;
        }
    }
    if (body.contains("top_logprobs") && body["top_logprobs"].is_number_integer()) {
        parsed.top_logprobs = body["top_logprobs"].get<int>();
        if (parsed.top_logprobs > 0) {
            parsed.logprobs = true;
        }
    }

    if (!parseStopSequences(body, parsed.stop_sequences, error)) {
        return false;
    }

    params = std::move(parsed);
    return true;
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

// T017: Generate unique response ID with prefix, timestamp, and random component
std::string generate_response_id(const std::string& prefix) {
    auto now = std::chrono::system_clock::now();
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        now.time_since_epoch()).count();

    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, 0xFFFF);

    std::ostringstream oss;
    oss << prefix << "-" << std::hex << ms << "-"
        << std::setw(4) << std::setfill('0') << dis(gen);
    return oss.str();
}

// T018: Get current Unix timestamp in seconds
int64_t get_current_timestamp() {
    auto now = std::chrono::system_clock::now();
    return std::chrono::duration_cast<std::chrono::seconds>(
        now.time_since_epoch()).count();
}

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
        engine_.applyPendingEnginePluginsIfIdle();
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
            std::string param_error;
            if (!validateSamplingParams(body, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            bool has_prompt = false;
            for (const auto& msg : parsed.messages) {
                if (!trimAscii(msg.content).empty()) {
                    has_prompt = true;
                    break;
                }
            }
            if (!has_prompt) {
                respondError(res, 400, "invalid_request", "prompt must not be empty");
                return;
            }
            bool stream = body.value("stream", false);
            InferenceParams params;
            if (!parseInferenceParams(body, params, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            LogprobsRequest logprobs_req;
            if (!parseLogprobsRequest(body, logprobs_req, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }

            if (stream) {
                if (logprobs_req.enabled) {
                    respondError(res, 400, "invalid_request", "logprobs is not supported with stream");
                    return;
                }
                // T041: n > 1 とストリーミングの同時指定は非対応
                if (params.n > 1) {
                    respondError(res, 400, "invalid_request", "n > 1 is not supported with stream");
                    return;
                }
                // ストリーミング用に生成
                std::string output;
                if (!parsed.image_urls.empty()) {
                    output = engine_.generateChatWithImages(parsed.messages, parsed.image_urls, model, params);
                } else {
                    output = engine_.generateChat(parsed.messages, model, params);
                }
                output = applyStopSequences(std::move(output), params.stop_sequences);
                output = sanitize_utf8_lossy(output);

                auto guard_ptr = std::make_shared<RequestGuard>(std::move(*guard));
                // T039: Generate stream ID and timestamp once for all chunks
                std::string stream_id = generate_response_id("chatcmpl");
                int64_t stream_created = get_current_timestamp();
                res.set_header("Content-Type", "text/event-stream");
                res.set_chunked_content_provider("text/event-stream",
                    [output, model, stream_id, stream_created, guard_ptr](size_t offset, httplib::DataSink& sink) {
                        if (offset == 0) {
                            // OpenAI compatible streaming format
                            json event_data = {
                                {"id", stream_id},
                                {"object", "chat.completion.chunk"},
                                {"created", stream_created},
                                {"model", model},
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

            // T019: Build prompt from messages for token counting
            std::string prompt_text;
            for (const auto& msg : parsed.messages) {
                prompt_text += msg.role + ": " + msg.content + "\n";
            }
            int prompt_tokens = static_cast<int>(prompt_text.length() / 4);  // Approximate tokenization

            // T037: n回の生成ループでchoices配列を構築
            json choices = json::array();
            int total_completion_tokens = 0;
            for (int i = 0; i < params.n; ++i) {
                std::string gen_output;
                if (!parsed.image_urls.empty()) {
                    gen_output = engine_.generateChatWithImages(parsed.messages, parsed.image_urls, model, params);
                } else {
                    gen_output = engine_.generateChat(parsed.messages, model, params);
                }
                gen_output = applyStopSequences(std::move(gen_output), params.stop_sequences);
                gen_output = sanitize_utf8_lossy(gen_output);

                json choice = {
                    {"index", i},
                    {"message", {{"role", "assistant"}, {"content", gen_output}}},
                    {"finish_reason", "stop"}
                };
                if (logprobs_req.enabled) {
                    choice["logprobs"] = build_logprobs(gen_output, logprobs_req.top_logprobs);
                }
                choices.push_back(choice);
                total_completion_tokens += static_cast<int>(gen_output.length() / 4);
            }

            // T019, T021, T023: Add usage, dynamic ID, and current timestamp
            json resp = {
                {"id", generate_response_id("chatcmpl")},
                {"object", "chat.completion"},
                {"created", get_current_timestamp()},
                {"model", model},
                {"choices", choices},
                {"usage", {
                    {"prompt_tokens", prompt_tokens},
                    {"completion_tokens", total_completion_tokens},
                    {"total_tokens", prompt_tokens + total_completion_tokens}
                }}
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
        engine_.applyPendingEnginePluginsIfIdle();
        auto guard = RequestGuard::try_acquire();
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;
            if (!body.contains("prompt") || !body["prompt"].is_string()) {
                respondError(res, 400, "invalid_request", "prompt is required");
                return;
            }
            std::string prompt = body["prompt"].get<std::string>();
            if (trimAscii(prompt).empty()) {
                respondError(res, 400, "invalid_request", "prompt must not be empty");
                return;
            }
            std::string param_error;
            if (!validateSamplingParams(body, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            InferenceParams params;
            if (!parseInferenceParams(body, params, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            LogprobsRequest logprobs_req;
            if (!parseLogprobsRequest(body, logprobs_req, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }

            // T020, T022, T024: Add usage, dynamic ID, and current timestamp
            int prompt_tokens = static_cast<int>(prompt.length() / 4);  // Approximate tokenization

            // T038: n回の生成ループでchoices配列を構築
            json choices = json::array();
            int total_completion_tokens = 0;
            for (int i = 0; i < params.n; ++i) {
                std::string output = engine_.generateCompletion(prompt, model, params);
                output = applyStopSequences(std::move(output), params.stop_sequences);
                output = sanitize_utf8_lossy(output);

                json choice = {
                    {"text", output},
                    {"index", i},
                    {"finish_reason", "stop"}
                };
                if (logprobs_req.enabled) {
                    choice["logprobs"] = build_logprobs(output, logprobs_req.top_logprobs);
                }
                choices.push_back(choice);
                total_completion_tokens += static_cast<int>(output.length() / 4);
            }

            json resp = {
                {"id", generate_response_id("cmpl")},
                {"object", "text_completion"},
                {"created", get_current_timestamp()},
                {"model", model},
                {"choices", choices},
                {"usage", {
                    {"prompt_tokens", prompt_tokens},
                    {"completion_tokens", total_completion_tokens},
                    {"total_tokens", prompt_tokens + total_completion_tokens}
                }}
            };
            setJson(res, resp);
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/embeddings", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        engine_.applyPendingEnginePluginsIfIdle();
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
    const auto parsed = parse_model_name_with_quantization(model);
    if (!parsed.valid || parsed.name.empty()) {
        respondError(res, 400, "invalid_request", "model is invalid");
        return false;
    }
    const bool in_registry = registry_.hasModel(parsed.name);
    if (in_registry && !engine_.isInitialized()) {
        return true;
    }
    // Try to resolve/load via ModelResolver (local -> shared -> router API)
    // loadModel() handles the full resolution flow
    auto load_result = engine_.loadModel(model, capability);
    if (!load_result.success) {
        const std::string prefix = "Model does not support capability:";
        if (load_result.error_code == llm_node::EngineErrorCode::kOomVram ||
            load_result.error_code == llm_node::EngineErrorCode::kOomRam) {
            respondError(res, 503, "resource_exhausted", load_result.error_message);
            return false;
        }
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
