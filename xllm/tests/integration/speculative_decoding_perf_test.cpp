#include <gtest/gtest.h>
#include <httplib.h>
#include <chrono>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "core/inference_engine.h"
#include "models/model_registry.h"
#include "runtime/state.h"

using namespace xllm;

TEST(SpeculativeDecodingPerfTest, SpeculativeRequestCompletesWithinBudget) {
    xllm::set_ready(true);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});

    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::Cpu);
    NodeEndpoints node;
    HttpServer server(18151, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18151);

    const std::string baseline_body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}]})";
    const std::string speculative_body = R"({
        "model":"gpt-oss-7b",
        "messages":[{"role":"user","content":"hello"}],
        "draft_model":"gpt-oss-7b",
        "speculative":{"max_tokens":8,"min_tokens":1}
    })";

    auto start_baseline = std::chrono::steady_clock::now();
    auto baseline = cli.Post("/v1/chat/completions", baseline_body, "application/json");
    auto end_baseline = std::chrono::steady_clock::now();

    auto start_spec = std::chrono::steady_clock::now();
    auto speculative = cli.Post("/v1/chat/completions", speculative_body, "application/json");
    auto end_spec = std::chrono::steady_clock::now();

    ASSERT_TRUE(baseline);
    ASSERT_TRUE(speculative);
    EXPECT_EQ(baseline->status, 200);
    EXPECT_EQ(speculative->status, 200);

    const auto baseline_ms = std::chrono::duration_cast<std::chrono::milliseconds>(end_baseline - start_baseline).count();
    const auto spec_ms = std::chrono::duration_cast<std::chrono::milliseconds>(end_spec - start_spec).count();

    // Stub engine should respond quickly; allow generous headroom to avoid flakes.
    EXPECT_LT(baseline_ms, 2000);
    EXPECT_LT(spec_ms, 2000);

    server.stop();
}
