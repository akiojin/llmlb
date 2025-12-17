#include <gtest/gtest.h>
#include <httplib.h>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "utils/config.h"
#include "runtime/state.h"

using namespace llm_node;

TEST(OpenAIEndpointsTest, ListsModelsAndRespondsToChat) {
    llm_node::set_ready(true);  // Ensure node is ready
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18087, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18087);
    auto models = cli.Get("/v1/models");
    ASSERT_TRUE(models);
    EXPECT_EQ(models->status, 200);
    EXPECT_NE(models->body.find("gpt-oss-7b"), std::string::npos);

    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}]})";
    auto chat = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(chat);
    EXPECT_EQ(chat->status, 200);
    EXPECT_NE(chat->body.find("Response to"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, Returns404WhenModelMissing) {
    llm_node::set_ready(true);  // Ensure node is ready
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18092, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18092);
    std::string body = R"({"model":"missing","prompt":"hello"})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 404);
    EXPECT_NE(res->body.find("model_not_found"), std::string::npos);

    server.stop();
}

// SPEC-dcaeaec4: Node returns 503 when not ready (syncing with router)
TEST(OpenAIEndpointsTest, Returns503WhenNotReady) {
    // Set node to not ready state
    llm_node::set_ready(false);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18093, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18093);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 503);
    EXPECT_NE(res->body.find("service_unavailable"), std::string::npos);

    server.stop();
    llm_node::set_ready(true);  // Cleanup for other tests
}

// SPEC-dcaeaec4: Completions endpoint returns 503 when not ready
TEST(OpenAIEndpointsTest, CompletionsReturns503WhenNotReady) {
    llm_node::set_ready(false);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18094, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18094);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello"})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 503);
    EXPECT_NE(res->body.find("service_unavailable"), std::string::npos);

    server.stop();
    llm_node::set_ready(true);
}

// SPEC-dcaeaec4: Embeddings endpoint returns 503 when not ready
TEST(OpenAIEndpointsTest, EmbeddingsReturns503WhenNotReady) {
    llm_node::set_ready(false);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18095, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18095);
    std::string body = R"({"model":"gpt-oss-7b","input":"hello"})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 503);
    EXPECT_NE(res->body.find("service_unavailable"), std::string::npos);

    server.stop();
    llm_node::set_ready(true);
}

// Invalid JSON handling
TEST(OpenAIEndpointsTest, ReturnsErrorOnInvalidJSON) {
    llm_node::set_ready(true);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18096, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18096);
    std::string invalid_json = R"({invalid json here)";
    auto res = cli.Post("/v1/chat/completions", invalid_json, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);

    server.stop();
}

// Missing required field (model)
TEST(OpenAIEndpointsTest, ReturnsErrorOnMissingModel) {
    llm_node::set_ready(true);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18097, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18097);
    // Missing "model" field
    std::string body = R"({"messages":[{"role":"user","content":"hello"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);

    server.stop();
}
