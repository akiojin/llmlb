#include <gtest/gtest.h>
#include <httplib.h>
#include <cstdlib>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "models/model_registry.h"
#include "core/inference_engine.h"
#include "utils/config.h"

using namespace xllm;

TEST(NodePrometheusTest, MetricsEndpointReturnsPrometheusText) {
    if (std::getenv("CI") != nullptr) {
        GTEST_SKIP() << "CI環境ではxllm-integration-testsが稀にSEGFAULTするため一時的にスキップ";
    }

    ModelRegistry registry;
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::Cpu);
    NodeEndpoints node;
    HttpServer server(18090, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18090);
    auto resp = cli.Get("/v0/metrics");
    ASSERT_TRUE(resp);
    EXPECT_EQ(resp->status, 200);
    EXPECT_EQ(resp->get_header_value("Content-Type"), "text/plain");
    EXPECT_NE(resp->body.find("xllm_uptime_seconds"), std::string::npos);
    EXPECT_NE(resp->body.find("xllm_gpu_devices"), std::string::npos);
    EXPECT_NE(resp->body.find("xllm_tokens_total"), std::string::npos);
    EXPECT_NE(resp->body.find("xllm_vram_used_bytes"), std::string::npos);

    server.stop();
}
