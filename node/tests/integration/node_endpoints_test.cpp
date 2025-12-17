#include <gtest/gtest.h>
#include <httplib.h>
#include <nlohmann/json.hpp>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "runtime/state.h"
#include "utils/config.h"

using namespace llm_node;

TEST(NodeEndpointsTest, PullAndHealth) {
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18088, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18088);
    auto health = cli.Get("/health");
    ASSERT_TRUE(health);
    EXPECT_EQ(health->status, 200);
    EXPECT_NE(health->body.find("ok"), std::string::npos);

    server.stop();
}

TEST(NodeEndpointsTest, LogLevelGetAndSet) {
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18087, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18087);
    auto get1 = cli.Get("/log/level");
    ASSERT_TRUE(get1);
    EXPECT_EQ(get1->status, 200);
    auto j1 = nlohmann::json::parse(get1->body);
    EXPECT_TRUE(j1.contains("level"));

    auto set = cli.Post("/log/level", R"({"level":"debug"})", "application/json");
    ASSERT_TRUE(set);
    EXPECT_EQ(set->status, 200);
    auto j2 = nlohmann::json::parse(set->body);
    EXPECT_EQ(j2.value("status", ""), "ok");
    EXPECT_EQ(j2.value("level", ""), "debug");

    auto get2 = cli.Get("/log/level");
    ASSERT_TRUE(get2);
    EXPECT_EQ(get2->status, 200);
    auto j3 = nlohmann::json::parse(get2->body);
    EXPECT_EQ(j3.value("level", ""), "debug");

    server.stop();
}

TEST(NodeEndpointsTest, StartupProbeReflectsReadyFlag) {
    llm_node::set_ready(false);
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18091, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18091);
    auto not_ready = cli.Get("/startup");
    ASSERT_TRUE(not_ready);
    EXPECT_EQ(not_ready->status, 503);

    llm_node::set_ready(true);
    auto ready = cli.Get("/startup");
    ASSERT_TRUE(ready);
    EXPECT_EQ(ready->status, 200);

    server.stop();
}

TEST(NodeEndpointsTest, MetricsReportsUptimeAndCounts) {
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18089, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18089);

    auto metrics = cli.Get("/metrics");
    ASSERT_TRUE(metrics);
    EXPECT_EQ(metrics->status, 200);
    EXPECT_EQ(metrics->get_header_value("Content-Type"), "application/json");
    auto body = nlohmann::json::parse(metrics->body);
    EXPECT_TRUE(body.contains("uptime_seconds"));
    EXPECT_TRUE(body.contains("gpu_devices"));
    EXPECT_TRUE(body.contains("request_count"));
    EXPECT_TRUE(body.contains("pull_count"));

    server.stop();
}

TEST(HttpServerTest, RequestIdGeneratedAndEchoed) {
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18092, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18092);
    auto resp = cli.Get("/health");
    ASSERT_TRUE(resp);
    auto id = resp->get_header_value("X-Request-Id");
    EXPECT_FALSE(id.empty());

    // Custom request id is echoed
    httplib::Headers h{{"X-Request-Id", "custom-id"}};
    auto resp2 = cli.Get("/health", h);
    ASSERT_TRUE(resp2);
    EXPECT_EQ(resp2->get_header_value("X-Request-Id"), "custom-id");

    server.stop();
}

TEST(HttpServerTest, TraceparentPropagatesTraceId) {
    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18093, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18093);
    std::string incoming = "00-11111111111111111111111111111111-2222222222222222-01";
    httplib::Headers h{{"traceparent", incoming}};
    auto resp = cli.Get("/health", h);
    ASSERT_TRUE(resp);
    auto tp = resp->get_header_value("traceparent");
    EXPECT_FALSE(tp.empty());
    EXPECT_NE(tp.find("11111111111111111111111111111111"), std::string::npos);
    EXPECT_EQ(tp.size(), 55);
    server.stop();
}
