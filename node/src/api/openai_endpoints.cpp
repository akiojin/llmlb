#include "api/openai_endpoints.h"

#include <nlohmann/json.hpp>
#include "models/model_registry.h"
#include "core/inference_engine.h"

namespace llm_node {

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
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, res)) return;
            std::vector<ChatMessage> messages;
            if (body.contains("messages")) {
                for (const auto& m : body["messages"]) {
                    messages.push_back({m.value("role", ""), m.value("content", "")});
                }
            }
            bool stream = body.value("stream", false);
            std::string output = engine_.generateChat(messages, model);

            if (stream) {
                res.set_header("Content-Type", "text/event-stream");
                res.set_chunked_content_provider("text/event-stream",
                    [output](size_t offset, httplib::DataSink& sink) {
                        if (offset == 0) {
                            json event_data = {{"content", output}};
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
    if (!registry_.hasModel(model)) {
        respondError(res, 404, "model_not_found", "model not found");
        return false;
    }
    return true;
}

}  // namespace llm_node
