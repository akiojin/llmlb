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
    OpenAIEndpoints openai{registry, engine, config};
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

// T003: ChatCompletions returns usage field with valid token counts
TEST_F(OpenAIContractFixture, ChatCompletionsReturnsUsage) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j.contains("usage"));
    EXPECT_GT(j["usage"]["prompt_tokens"].get<int>(), 0);
    EXPECT_GT(j["usage"]["completion_tokens"].get<int>(), 0);
    EXPECT_EQ(j["usage"]["total_tokens"].get<int>(),
              j["usage"]["prompt_tokens"].get<int>() + j["usage"]["completion_tokens"].get<int>());
}

// T004: Completions returns usage field with valid token counts
TEST_F(OpenAIContractFixture, CompletionsReturnsUsage) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello world"})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j.contains("usage"));
    EXPECT_GT(j["usage"]["prompt_tokens"].get<int>(), 0);
    EXPECT_GT(j["usage"]["completion_tokens"].get<int>(), 0);
    EXPECT_EQ(j["usage"]["total_tokens"].get<int>(),
              j["usage"]["prompt_tokens"].get<int>() + j["usage"]["completion_tokens"].get<int>());
}

// T005: Response ID is unique across requests
TEST_F(OpenAIContractFixture, ResponseIdIsUnique) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}]})";

    auto res1 = cli.Post("/v1/chat/completions", body, "application/json");
    auto res2 = cli.Post("/v1/chat/completions", body, "application/json");

    ASSERT_TRUE(res1);
    ASSERT_TRUE(res2);
    EXPECT_EQ(res1->status, 200);
    EXPECT_EQ(res2->status, 200);

    auto j1 = json::parse(res1->body);
    auto j2 = json::parse(res2->body);

    std::string id1 = j1["id"];
    std::string id2 = j2["id"];

    EXPECT_NE(id1, id2);
    EXPECT_TRUE(id1.find("chatcmpl-") == 0);
    EXPECT_TRUE(id2.find("chatcmpl-") == 0);
}

// T006: Created timestamp is a valid Unix timestamp
TEST_F(OpenAIContractFixture, CreatedTimestampIsValid) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);

    ASSERT_TRUE(j.contains("created"));
    int64_t created = j["created"].get<int64_t>();
    // Should be a reasonable Unix timestamp (after 2020-01-01)
    EXPECT_GT(created, 1577836800);
    // Should not be in the far future (before 2100-01-01)
    EXPECT_LT(created, 4102444800);
}

// T007: presence_penalty is accepted
TEST_F(OpenAIContractFixture, PresencePenaltyAccepted) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"presence_penalty":0.5})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
}

// T008: frequency_penalty is accepted
TEST_F(OpenAIContractFixture, FrequencyPenaltyAccepted) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"frequency_penalty":0.5})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
}

// T009: Penalty out of range returns 400
TEST_F(OpenAIContractFixture, PenaltyOutOfRangeReturns400) {
    httplib::Client cli("127.0.0.1", 18090);

    // presence_penalty too low
    std::string body1 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"presence_penalty":-2.5})";
    auto res1 = cli.Post("/v1/chat/completions", body1, "application/json");
    ASSERT_TRUE(res1);
    EXPECT_EQ(res1->status, 400);

    // presence_penalty too high
    std::string body2 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"presence_penalty":2.5})";
    auto res2 = cli.Post("/v1/chat/completions", body2, "application/json");
    ASSERT_TRUE(res2);
    EXPECT_EQ(res2->status, 400);

    // frequency_penalty too low
    std::string body3 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"frequency_penalty":-2.5})";
    auto res3 = cli.Post("/v1/chat/completions", body3, "application/json");
    ASSERT_TRUE(res3);
    EXPECT_EQ(res3->status, 400);

    // frequency_penalty too high
    std::string body4 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"frequency_penalty":2.5})";
    auto res4 = cli.Post("/v1/chat/completions", body4, "application/json");
    ASSERT_TRUE(res4);
    EXPECT_EQ(res4->status, 400);
}

// T010: Logprobs returns real values (not just 0.0)
TEST_F(OpenAIContractFixture, LogprobsReturnsRealValues) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello world","logprobs":true})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j["choices"][0].contains("logprobs"));
    auto logprobs = j["choices"][0]["logprobs"];
    ASSERT_TRUE(logprobs["token_logprobs"].is_array());
    EXPECT_FALSE(logprobs["token_logprobs"].empty());
    // Check that logprobs are real values (negative numbers, not 0.0)
    for (const auto& lp : logprobs["token_logprobs"]) {
        if (!lp.is_null()) {
            float val = lp.get<float>();
            EXPECT_LT(val, 0.0f) << "logprob should be negative";
        }
    }
}

// T011: top_logprobs returns N items
TEST_F(OpenAIContractFixture, TopLogprobsReturnsNItems) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello world","logprobs":true,"top_logprobs":3})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j["choices"][0].contains("logprobs"));
    auto logprobs = j["choices"][0]["logprobs"];
    ASSERT_TRUE(logprobs["top_logprobs"].is_array());
    EXPECT_FALSE(logprobs["top_logprobs"].empty());
    // Each entry should have up to 3 items
    for (const auto& top : logprobs["top_logprobs"]) {
        if (!top.is_null() && top.is_object()) {
            EXPECT_LE(top.size(), 3);
        }
    }
}

// T012: n parameter returns multiple choices
TEST_F(OpenAIContractFixture, NParameterReturnsMultipleChoices) {
    httplib::Client cli("127.0.0.1", 18090);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"n":3})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    auto j = json::parse(res->body);
    ASSERT_TRUE(j.contains("choices"));
    EXPECT_EQ(j["choices"].size(), 3);
    // Each choice should have a different index
    EXPECT_EQ(j["choices"][0]["index"].get<int>(), 0);
    EXPECT_EQ(j["choices"][1]["index"].get<int>(), 1);
    EXPECT_EQ(j["choices"][2]["index"].get<int>(), 2);
}

// T013: n parameter out of range returns 400
TEST_F(OpenAIContractFixture, NParameterOutOfRangeReturns400) {
    httplib::Client cli("127.0.0.1", 18090);

    // n too low
    std::string body1 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"n":0})";
    auto res1 = cli.Post("/v1/chat/completions", body1, "application/json");
    ASSERT_TRUE(res1);
    EXPECT_EQ(res1->status, 400);

    // n too high
    std::string body2 = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"test"}],"n":10})";
    auto res2 = cli.Post("/v1/chat/completions", body2, "application/json");
    ASSERT_TRUE(res2);
    EXPECT_EQ(res2->status, 400);
}
