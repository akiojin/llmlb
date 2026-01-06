#include <gtest/gtest.h>
#include <httplib.h>
#include <nlohmann/json.hpp>
#include <thread>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "utils/config.h"
#include "runtime/state.h"

using namespace llm_node;
using json = nlohmann::json;

class OpenAIContractFixture : public ::testing::Test {
protected:
    void SetUp() override {
        llm_node::set_ready(true);  // Ensure node is ready for contract tests
        registry.setModels({"gpt-oss-7b"});
        server = std::make_unique<HttpServer>(18090, openai, node);
        server->start();
    }

    void TearDown() override {
        server->stop();
    }

    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai{registry, engine, config, GpuBackend::kCpu};
    NodeEndpoints node;
    std::unique_ptr<HttpServer> server;
};

TEST_F(OpenAIContractFixture, ModelsEndpointReturnsArray) {
    httplib::Client cli("127.0.0.1", 18090);
    auto res = cli.Get("/v1/models");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto body = json::parse(res->body);
    ASSERT_TRUE(body.contains("data"));
    EXPECT_FALSE(body["data"].empty());
}

TEST_F(OpenAIContractFixture, ChatCompletionsReturnsMessage) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"ping"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    EXPECT_EQ(j["object"], "chat.completion");
    EXPECT_EQ(j["choices"][0]["message"]["role"], "assistant");
}

TEST_F(OpenAIContractFixture, ChatCompletionsSupportsStreamingSSE) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"stream"}],"stream":true})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    // SSE should include data: prefix
    EXPECT_NE(res->body.find("data:"), std::string::npos);
    EXPECT_NE(res->body.find("[DONE]"), std::string::npos);
    EXPECT_EQ(res->get_header_value("Content-Type"), "text/event-stream");
}

TEST_F(OpenAIContractFixture, EmbeddingsReturnsVectorWithSingleInput) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","input":"Hello, world!"})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    EXPECT_EQ(j["object"], "list");
    ASSERT_TRUE(j.contains("data"));
    EXPECT_FALSE(j["data"].empty());
    EXPECT_EQ(j["data"][0]["object"], "embedding");
    EXPECT_EQ(j["data"][0]["index"], 0);
    ASSERT_TRUE(j["data"][0]["embedding"].is_array());
    EXPECT_FALSE(j["data"][0]["embedding"].empty());
}

TEST_F(OpenAIContractFixture, EmbeddingsReturnsVectorsWithArrayInput) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","input":["Hello","World"]})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    EXPECT_EQ(j["object"], "list");
    // 2つの入力に対して2つのembeddingを返す
    EXPECT_EQ(j["data"].size(), 2);
    EXPECT_EQ(j["data"][0]["index"], 0);
    EXPECT_EQ(j["data"][1]["index"], 1);
}

TEST_F(OpenAIContractFixture, EmbeddingsRequiresInput) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b"})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, EmbeddingsRequiresModel) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"input":"Hello"})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, CompletionsRejectsEmptyPrompt) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"   "})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, CompletionsRejectsTemperatureOutOfRange) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hi","temperature":-0.5})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, CompletionsRejectsTopPOutOfRange) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hi","top_p":1.5})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, CompletionsRejectsTopKOutOfRange) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hi","top_k":-1})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
}

TEST_F(OpenAIContractFixture, CompletionsReturnsLogprobsWhenRequested) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello world","logprobs":true,"top_logprobs":1})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j["choices"][0].contains("logprobs"));
    auto logprobs = j["choices"][0]["logprobs"];
    ASSERT_TRUE(logprobs.is_object());
    ASSERT_TRUE(logprobs["tokens"].is_array());
    ASSERT_TRUE(logprobs["token_logprobs"].is_array());
    ASSERT_TRUE(logprobs["top_logprobs"].is_array());
    EXPECT_EQ(logprobs["tokens"].size(), logprobs["token_logprobs"].size());
    EXPECT_EQ(logprobs["tokens"].size(), logprobs["top_logprobs"].size());
    EXPECT_GT(logprobs["tokens"].size(), 0);
}

TEST_F(OpenAIContractFixture, ChatCompletionsAppliesStopSequence) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"ping STOP pong"}],"stop":"STOP"})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    std::string content = j["choices"][0]["message"]["content"];
    EXPECT_EQ(content, "Response to: ping ");
}

TEST_F(OpenAIContractFixture, CompletionsAppliesStopSequenceArray) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello STOP world","stop":["STOP","END"]})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    std::string text = j["choices"][0]["text"];
    EXPECT_EQ(text, "Response to: hello ");
}
