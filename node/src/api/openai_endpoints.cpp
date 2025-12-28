#include "api/openai_endpoints.h"

#include <nlohmann/json.hpp>
#include <memory>
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "runtime/state.h"

namespace llm_node {

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
}  // namespace

using json = nlohmann::json;

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
            if (!validateModel(model, res)) return;
            ParsedChatMessages parsed;
            std::string parse_error;
            if (!parseChatMessages(body, parsed, parse_error)) {
                respondError(res, 400, "bad_request", parse_error);
                return;
            }
            bool stream = body.value("stream", false);
            std::string output;
            if (!parsed.image_urls.empty()) {
                output = engine_.generateChatWithImages(parsed.messages, parsed.image_urls, model);
            } else {
                output = engine_.generateChat(parsed.messages, model);
            }

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
            if (!validateModel(model, res)) return;
            std::string prompt = body.value("prompt", "");
            std::string output = engine_.generateCompletion(prompt, model);
            json resp = {
                {"id", "cmpl-1"},
                {"object", "text_completion"},
                {"choices", json::array({{{"text", output}, {"index", 0}, {"finish_reason", "stop"}}})}
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
            if (!validateModel(model, res)) return;

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

bool OpenAIEndpoints::validateModel(const std::string& model, httplib::Response& res) {
    if (model.empty()) {
        respondError(res, 400, "model_required", "model is required");
        return false;
    }
    // Check local registry first
    if (registry_.hasModel(model)) {
        return true;
    }
    // Try to resolve/load via ModelResolver (local -> shared -> router API)
    // loadModel() handles the full resolution flow
    auto load_result = engine_.loadModel(model);
    if (!load_result.success) {
        respondError(res, 404, "model_not_found",
            load_result.error_message.empty() ? "model not found" : load_result.error_message);
        return false;
    }
    return true;
}

}  // namespace llm_node
