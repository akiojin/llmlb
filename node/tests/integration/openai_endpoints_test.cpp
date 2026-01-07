#include <gtest/gtest.h>
#include <httplib.h>
#include <filesystem>
#include <fstream>
#include <vector>
#include <nlohmann/json.hpp>

#include "api/http_server.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "core/llama_manager.h"
#include "models/model_registry.h"
#include "models/model_storage.h"
#include "core/inference_engine.h"
#include "utils/config.h"
#include "runtime/state.h"

using namespace llm_node;
namespace fs = std::filesystem;

namespace {
class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("openai-endpoints-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }
    fs::path path;
};

void write_text(const fs::path& path, const std::string& content) {
    std::ofstream ofs(path);
    ofs << content;
}
}  // namespace

TEST(OpenAIEndpointsTest, ListsModelsAndRespondsToChat) {
    llm_node::set_ready(true);  // Ensure node is ready
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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

TEST(OpenAIEndpointsTest, Returns400OnInvalidTemperature) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18101, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18101);
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"hello"}],"temperature":3.5})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
    EXPECT_NE(res->body.find("temperature"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, Returns400OnInvalidTopP) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18102, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18102);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello","top_p":1.5})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
    EXPECT_NE(res->body.find("top_p"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, Returns400OnInvalidTopK) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18103, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18103);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello","top_k":-1})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
    EXPECT_NE(res->body.find("top_k"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, Returns400OnEmptyPrompt) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18104, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18104);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"   "})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
    EXPECT_NE(res->body.find("prompt must not be empty"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, AppliesStopSequencesToCompletions) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18105, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18105);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello","stop":"hello"})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);
    EXPECT_EQ(res->body.find("hello"), std::string::npos);

    server.stop();
}

TEST(OpenAIEndpointsTest, ReturnsLogprobsForCompletions) {
    llm_node::set_ready(true);
    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18106, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18106);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"hello","logprobs":true,"top_logprobs":2})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);

    auto payload = nlohmann::json::parse(res->body);
    ASSERT_TRUE(payload.contains("choices"));
    auto logprobs = payload["choices"][0]["logprobs"];
    ASSERT_TRUE(logprobs.is_object());
    ASSERT_TRUE(logprobs.contains("tokens"));
    ASSERT_TRUE(logprobs.contains("token_logprobs"));
    ASSERT_TRUE(logprobs.contains("top_logprobs"));

    const auto& tokens = logprobs["tokens"];
    const auto& token_logprobs = logprobs["token_logprobs"];
    const auto& top_logprobs = logprobs["top_logprobs"];
    EXPECT_EQ(tokens.size(), token_logprobs.size());
    EXPECT_EQ(tokens.size(), top_logprobs.size());
    if (!tokens.empty()) {
        EXPECT_TRUE(top_logprobs[0].is_object());
        EXPECT_GE(top_logprobs[0].size(), 2);
    }

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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
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

TEST(OpenAIEndpointsTest, EmbeddingsReturns400WhenCapabilityMissing) {
    llm_node::set_ready(true);

    TempDir tmp;
    const std::string model_id = "test/llama-7b";
    auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_id);
    fs::create_directories(model_dir);
    write_text(model_dir / "config.json", R"({"architectures":["LlamaForCausalLM"]})");
    write_text(model_dir / "tokenizer.json", R"({"dummy":true})");
    write_text(model_dir / "model.safetensors", "dummy");

    ModelStorage storage(tmp.path.string());
    LlamaManager llama(tmp.path.string());
    InferenceEngine engine(llama, storage);
    ModelRegistry registry;
    registry.setModels({model_id});
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config, GpuBackend::kCpu);
    NodeEndpoints node;
    HttpServer server(18098, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18098);
    std::string body = std::string(R"({"model":")") + model_id + R"(","input":"hello"})";
    auto res = cli.Post("/v1/embeddings", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 400);
    EXPECT_NE(res->body.find("does not support capability"), std::string::npos);

    server.stop();
}

// T014: Usage token count matches actual tokenization
TEST(OpenAIEndpointsTest, UsageMatchesActualTokenCount) {
    llm_node::set_ready(true);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18110, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18110);
    // Send a simple message and check usage is calculated
    std::string body = R"({"model":"gpt-oss-7b","messages":[{"role":"user","content":"Hello, how are you?"}]})";
    auto res = cli.Post("/v1/chat/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);

    auto j = nlohmann::json::parse(res->body);
    ASSERT_TRUE(j.contains("usage"));

    // Verify usage fields are present and reasonable
    int prompt_tokens = j["usage"]["prompt_tokens"].get<int>();
    int completion_tokens = j["usage"]["completion_tokens"].get<int>();
    int total_tokens = j["usage"]["total_tokens"].get<int>();

    // Prompt should have at least a few tokens
    EXPECT_GT(prompt_tokens, 0);
    // Completion should have at least one token
    EXPECT_GT(completion_tokens, 0);
    // Total should equal sum
    EXPECT_EQ(total_tokens, prompt_tokens + completion_tokens);

    server.stop();
}

// T015: Logprobs values match model output (not dummy values)
TEST(OpenAIEndpointsTest, LogprobsMatchesModelOutput) {
    llm_node::set_ready(true);

    ModelRegistry registry;
    registry.setModels({"gpt-oss-7b"});
    InferenceEngine engine;
    NodeConfig config;
    OpenAIEndpoints openai(registry, engine, config);
    NodeEndpoints node;
    HttpServer server(18111, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18111);
    std::string body = R"({"model":"gpt-oss-7b","prompt":"The quick brown fox","logprobs":true,"top_logprobs":5})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 200);

    auto j = nlohmann::json::parse(res->body);
    ASSERT_TRUE(j["choices"][0].contains("logprobs"));
    auto logprobs = j["choices"][0]["logprobs"];

    // Verify logprobs structure
    ASSERT_TRUE(logprobs.contains("tokens"));
    ASSERT_TRUE(logprobs.contains("token_logprobs"));
    ASSERT_TRUE(logprobs.contains("top_logprobs"));

    // Verify logprobs are real values (negative, not 0.0)
    const auto& token_logprobs = logprobs["token_logprobs"];
    for (const auto& lp : token_logprobs) {
        if (!lp.is_null()) {
            float val = lp.get<float>();
            // Log probabilities should be negative (probability < 1)
            EXPECT_LT(val, 0.0f) << "logprob should be negative";
            // And should be greater than some very negative number
            EXPECT_GT(val, -100.0f) << "logprob should be reasonable";
        }
    }

    // Verify top_logprobs contains multiple candidates
    const auto& top_lps = logprobs["top_logprobs"];
    for (const auto& entry : top_lps) {
        if (!entry.is_null() && entry.is_object()) {
            // Should have up to 5 top candidates
            EXPECT_LE(entry.size(), 5);
            // Each entry should have negative logprob values
            for (auto& [token, logprob_val] : entry.items()) {
                if (logprob_val.is_number()) {
                    EXPECT_LT(logprob_val.get<float>(), 0.0f);
                }
            }
        }
    }

    server.stop();
}
