#include <gtest/gtest.h>
#include <atomic>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <optional>
#include <stdexcept>
#include <thread>

#include "core/inference_engine.h"
#include "core/llama_manager.h"
#include "core/engine_registry.h"
#include "core/engine_error.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "api/http_server.h"
#include "models/model_registry.h"
#include "models/model_descriptor.h"
#include "models/model_storage.h"
#include "system/resource_monitor.h"
#include "runtime/state.h"

using namespace allm;
namespace fs = std::filesystem;


class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path();
        for (int i = 0; i < 10; ++i) {
            auto candidate = base / fs::path("engine-" + std::to_string(std::rand()));
            std::error_code ec;
            if (fs::create_directories(candidate, ec)) {
                path = candidate;
                return;
            }
        }
        path = base;
    }
    ~TempDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }
    fs::path path;
};

class RecordingEngine final : public Engine {
public:
    RecordingEngine(std::string runtime,
                    std::string name,
                    std::vector<std::string>* calls,
                    bool supports_text,
                    bool supports_embeddings)
        : runtime_(std::move(runtime))
        , name_(std::move(name))
        , calls_(calls)
        , supports_text_(supports_text)
        , supports_embeddings_(supports_embeddings) {}

    std::string runtime() const override { return runtime_; }
    bool supportsTextGeneration() const override { return supports_text_; }
    bool supportsEmbeddings() const override { return supports_embeddings_; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        if (calls_) calls_->push_back("load:" + name_);
        ModelLoadResult result;
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams&) const override {
        return "ok";
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        return "ok";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        if (calls_) calls_->push_back("embeddings:" + name_);
        return {{1.0f, 0.0f}};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }

private:
    std::string runtime_;
    std::string name_;
    std::vector<std::string>* calls_{nullptr};
    bool supports_text_{false};
    bool supports_embeddings_{false};
};

class VramEngine final : public Engine {
public:
    explicit VramEngine(uint64_t required) : required_(required) {}

    std::string runtime() const override { return "llama_cpp"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        ModelLoadResult result;
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams&) const override {
        return "ok";
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        return "ok";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        return {{1.0f, 0.0f}};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }

    uint64_t getModelVramBytes(const ModelDescriptor&) const override { return required_; }

private:
    uint64_t required_{0};
};

class BlockingEngine final : public Engine {
public:
    explicit BlockingEngine(std::atomic<bool>* allow_return)
        : allow_return_(allow_return) {}

    std::string runtime() const override { return "llama_cpp"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        ModelLoadResult result;
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams&) const override {
        while (!allow_return_->load()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        return "done";
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        return "done";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        return {{1.0f, 0.0f}};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }

private:
    std::atomic<bool>* allow_return_{nullptr};
};

class MetricsEngine final : public Engine {
public:
    MetricsEngine(uint64_t first_ns, uint64_t last_ns, bool* saw_callback)
        : first_ns_(first_ns)
        , last_ns_(last_ns)
        , saw_callback_(saw_callback) {}

    std::string runtime() const override { return "llama_cpp"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        ModelLoadResult result;
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams& params) const override {
        if (params.on_token_callback) {
            if (saw_callback_) {
                *saw_callback_ = true;
            }
            params.on_token_callback(params.on_token_callback_ctx, 1, first_ns_);
            params.on_token_callback(params.on_token_callback_ctx, 2, last_ns_);
        }
        return "ok";
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        return "ok";
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        return {};
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        return {{1.0f, 0.0f}};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }

private:
    uint64_t first_ns_{0};
    uint64_t last_ns_{0};
    bool* saw_callback_{nullptr};
};

class ThrowingEngine final : public Engine {
public:
    std::string runtime() const override { return "llama_cpp"; }
    bool supportsTextGeneration() const override { return true; }
    bool supportsEmbeddings() const override { return false; }

    ModelLoadResult loadModel(const ModelDescriptor&) override {
        ModelLoadResult result;
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    std::string generateChat(const std::vector<ChatMessage>&,
                             const ModelDescriptor&,
                             const InferenceParams&) const override {
        throw std::runtime_error("engine crash");
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams&) const override {
        throw std::runtime_error("engine crash");
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams&,
        const std::function<void(const std::string&)>&) const override {
        throw std::runtime_error("engine crash");
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        throw std::runtime_error("engine crash");
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override { return 0; }
};

TEST(InferenceEngineTest, GeneratesChatFromLastUserMessage) {
    InferenceEngine engine;
    std::vector<ChatMessage> msgs = {
        {"system", "You are a bot."},
        {"user", "Hello"},
        {"assistant", "Hi"},
        {"user", "How are you?"},
    };
    auto out = engine.generateChat(msgs, "dummy");
    EXPECT_NE(out.find("How are you?"), std::string::npos);
}

TEST(InferenceEngineTest, GeneratesCompletionFromPrompt) {
    InferenceEngine engine;
    auto out = engine.generateCompletion("Once upon a time", "dummy");
    EXPECT_NE(out.find("Once upon a time"), std::string::npos);
}

TEST(InferenceEngineTest, GeneratesTokensWithLimit) {
    InferenceEngine engine;
    auto tokens = engine.generateTokens("a b c d e f", 3);
    ASSERT_EQ(tokens.size(), 3u);
    EXPECT_EQ(tokens[0], "a");
    EXPECT_EQ(tokens[2], "c");
}

TEST(InferenceEngineTest, StreamsChatTokens) {
    InferenceEngine engine;
    std::vector<std::string> collected;
    std::vector<ChatMessage> msgs = {{"user", "hello stream test"}};
    auto tokens = engine.generateChatStream(msgs, 2, [&](const std::string& t) { collected.push_back(t); });
    ASSERT_EQ(tokens.size(), 2u);
    EXPECT_EQ(collected, tokens);
}

TEST(InferenceEngineTest, BatchGeneratesPerPrompt) {
    InferenceEngine engine;
    std::vector<std::string> prompts = {"one two", "alpha beta gamma"};
    auto outs = engine.generateBatch(prompts, 2);
    ASSERT_EQ(outs.size(), 2u);
    EXPECT_EQ(outs[0][0], "one");
    EXPECT_EQ(outs[1][1], "beta");
}

TEST(InferenceEngineTest, SampleNextTokenReturnsLast) {
    InferenceEngine engine;
    std::vector<std::string> tokens = {"x", "y", "z"};
    EXPECT_EQ(engine.sampleNextToken(tokens), "z");
}

TEST(InferenceEngineTest, LoadModelReturnsUnavailableWhenNotInitialized) {
    InferenceEngine engine;
    auto result = engine.loadModel("missing/model");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kInternal);
    EXPECT_NE(result.error_message.find("not initialized"), std::string::npos);
}

TEST(InferenceEngineTest, LoadModelReturnsNotFoundWhenMissingModel) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto result = engine.loadModel("missing/model");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kLoadFailed);
    EXPECT_NE(result.error_message.find("Model not found"), std::string::npos);
}

TEST(InferenceEngineTest, WatchdogTriggersTerminationOnTimeout) {
    TempDir tmp;
    const std::string model_name = "example/blocking";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    std::atomic<bool> allow_return{false};
    std::atomic<bool> timeout_fired{false};

    EngineRegistration reg;
    reg.engine_id = "blocking_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.architectures = {"llama"};
    reg.capabilities = {"text"};
    registry->registerEngine(std::make_unique<BlockingEngine>(&allow_return), reg, nullptr);
    engine.setEngineRegistryForTest(std::move(registry));

    InferenceEngine::setWatchdogTimeoutForTest(std::chrono::milliseconds(20));
    InferenceEngine::setWatchdogTerminateHookForTest([&]() {
        timeout_fired.store(true);
        allow_return.store(true);
    });

    std::thread worker([&]() {
        std::vector<ChatMessage> messages = {{"user", "hello"}};
        (void)engine.generateChat(messages, model_name, {});
    });

    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(1);
    while (!timeout_fired.load() && std::chrono::steady_clock::now() < deadline) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    if (!timeout_fired.load()) {
        allow_return.store(true);
    }
    worker.join();

    EXPECT_TRUE(timeout_fired.load());

    InferenceEngine::setWatchdogTimeoutForTest(std::chrono::seconds(30));
    InferenceEngine::setWatchdogTerminateHookForTest({});
}

TEST(InferenceEngineTest, LoadModelReturnsUnsupportedForCapability) {
    TempDir tmp;
    const std::string model_name = "example/model";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto result = engine.loadModel(model_name, "image");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kUnsupported);
    EXPECT_NE(result.error_message.find("capability"), std::string::npos);
}

TEST(InferenceEngineTest, LoadModelRejectsUnsupportedArchitecture) {
    TempDir tmp;
    const std::string model_name = "openai/gpt-oss-20b";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "config.json") << R"({"architectures":["GptOssForCausalLM"]})";
    std::ofstream(model_dir / "tokenizer.json") << R"({"dummy":true})";
    std::ofstream(model_dir / "model.safetensors") << "dummy";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "text_engine";
    reg.engine_version = "test";
    reg.formats = {"safetensors"};
    reg.architectures = {"llama"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("safetensors_cpp", "text", nullptr, true, false),
        reg,
        nullptr));

    engine.setEngineRegistryForTest(std::move(registry));

    auto result = engine.loadModel(model_name, "text");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kUnsupported);
    EXPECT_NE(result.error_message.find("architecture"), std::string::npos);
}

TEST(InferenceEngineTest, LoadModelRejectsWhenVramInsufficient) {
    TempDir tmp;
    const std::string model_name = "example/model";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "vram_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<VramEngine>(2048),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));
    engine.setResourceUsageProviderForTest([]() {
        return ResourceUsage{0, 0, 0, 1024};
    });

    auto result = engine.loadModel(model_name);
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kOomVram);
    EXPECT_NE(result.error_message.find("VRAM"), std::string::npos);
}

TEST(InferenceEngineTest, LoadModelRejectsWhenVramBudgetExceeded) {
    TempDir tmp;
    const std::string model_name = "example/model";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration primary_reg;
    primary_reg.engine_id = "budget_engine";
    primary_reg.engine_version = "test";
    primary_reg.formats = {"gguf"};
    primary_reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<VramEngine>(1536),
        primary_reg,
        nullptr));

    EngineRegistration other_reg;
    other_reg.engine_id = "other_engine";
    other_reg.engine_version = "test";
    other_reg.formats = {"gguf"};
    other_reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<VramEngine>(256),
        other_reg,
        nullptr));

    engine.setEngineRegistryForTest(std::move(registry));
    engine.setResourceUsageProviderForTest([]() {
        return ResourceUsage{0, 0, 0, 2048};
    });

    auto result = engine.loadModel(model_name);
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kOomVram);
    EXPECT_NE(result.error_message.find("budget"), std::string::npos);
}

TEST(InferenceEngineTest, OpenAIResponds503WhenVramInsufficient) {
    allm::set_ready(true);
    TempDir tmp;
    const std::string model_name = "example/model";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "vram_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<VramEngine>(2048),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));
    engine.setResourceUsageProviderForTest([]() {
        return ResourceUsage{0, 0, 0, 1024};
    });

    ModelRegistry api_registry;
    api_registry.setModels({model_name});
    NodeConfig config;
    OpenAIEndpoints openai(api_registry, engine, config, GpuBackend::Cpu);
    NodeEndpoints node;
    HttpServer server(18094, openai, node);
    server.start();

    httplib::Client cli("127.0.0.1", 18094);
    std::string body = R"({"model":"example/model","prompt":"hello"})";
    auto res = cli.Post("/v1/completions", body, "application/json");
    ASSERT_TRUE(res);
    EXPECT_EQ(res->status, 503);
    EXPECT_NE(res->body.find("resource_exhausted"), std::string::npos);

    server.stop();
}

TEST(InferenceEngineTest, LoadModelUsesCapabilityToResolveEngine) {
    TempDir tmp;
    const std::string model_name = "example/model";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    std::vector<std::string> calls;

    EngineRegistration text_reg;
    text_reg.engine_id = "text_engine";
    text_reg.engine_version = "test";
    text_reg.formats = {"gguf"};
    text_reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("llama_cpp", "text", &calls, true, false),
        text_reg,
        nullptr));

    EngineRegistration embed_reg;
    embed_reg.engine_id = "embed_engine";
    embed_reg.engine_version = "test";
    embed_reg.formats = {"gguf"};
    embed_reg.capabilities = {"embeddings"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("llama_cpp", "embed", &calls, false, true),
        embed_reg,
        nullptr));

    engine.setEngineRegistryForTest(std::move(registry));

    auto text_result = engine.loadModel(model_name, "text");
    EXPECT_TRUE(text_result.success);
    ASSERT_EQ(calls.size(), 1u);
    EXPECT_EQ(calls[0], "load:text");

    calls.clear();
    auto embed_result = engine.loadModel(model_name, "embeddings");
    EXPECT_TRUE(embed_result.success);
    ASSERT_EQ(calls.size(), 1u);
    EXPECT_EQ(calls[0], "load:embed");
}

TEST(InferenceEngineTest, LoadModelInvalidQuantizationReturnsUnsupportedError) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto result = engine.loadModel("example/model:");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kUnsupported);
}

TEST(InferenceEngineTest, LoadModelWithoutInitializationReturnsInternalError) {
    InferenceEngine engine;
    auto result = engine.loadModel("example/model");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error_code, EngineErrorCode::kInternal);
}

TEST(InferenceEngineTest, GenerateEmbeddingsUsesEmbeddingEngine) {
    TempDir tmp;
    const std::string model_name = "example/embed";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    std::vector<std::string> calls;

    EngineRegistration text_reg;
    text_reg.engine_id = "text_engine";
    text_reg.engine_version = "test";
    text_reg.formats = {"gguf"};
    text_reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("llama_cpp", "text", &calls, true, false),
        text_reg,
        nullptr));

    EngineRegistration embed_reg;
    embed_reg.engine_id = "embed_engine";
    embed_reg.engine_version = "test";
    embed_reg.formats = {"gguf"};
    embed_reg.capabilities = {"embeddings"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("llama_cpp", "embed", &calls, false, true),
        embed_reg,
        nullptr));

    engine.setEngineRegistryForTest(std::move(registry));

    auto embeddings = engine.generateEmbeddings({"hello"}, model_name);
    ASSERT_EQ(embeddings.size(), 1u);
    ASSERT_EQ(calls.size(), 1u);
    EXPECT_EQ(calls[0], "embeddings:embed");
}

// NOTE: gptoss/nemotron tests removed - these engines were removed during safetensors.cpp migration

TEST(InferenceParamsTest, ResolvesEffectiveMaxTokensFromContext) {
    EXPECT_EQ(resolve_effective_max_tokens(0, 10, 100), 90u);
    EXPECT_EQ(resolve_effective_max_tokens(5, 10, 100), 5u);
    EXPECT_EQ(resolve_effective_max_tokens(500, 10, 100), 90u);
    EXPECT_EQ(resolve_effective_max_tokens(kDefaultMaxTokens, 100, 8192), kDefaultMaxTokens);
    EXPECT_EQ(resolve_effective_max_tokens(0, 0, 0), kDefaultMaxTokens);
    EXPECT_EQ(resolve_effective_max_tokens(0, 100, 100), 0u);
    EXPECT_EQ(resolve_effective_max_tokens(5, 100, 100), 0u);
}

TEST(InferenceEngineTest, ComputesTokenMetricsFromCallback) {
    TempDir tmp;
    const std::string model_name = "example/metrics";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "metrics_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.architectures = {"llama"};
    reg.capabilities = {"text"};

    const uint64_t start_ns = 1'000'000'000ULL;
    const uint64_t first_ns = start_ns + 100'000'000ULL;
    const uint64_t last_ns = start_ns + 300'000'000ULL;
    bool saw_callback = false;
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<MetricsEngine>(first_ns, last_ns, &saw_callback),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    std::optional<TokenMetrics> captured;
    InferenceEngine::setTokenMetricsClockForTest([&]() { return start_ns; });
    InferenceEngine::setTokenMetricsHookForTest([&](const TokenMetrics& metrics) {
        captured = metrics;
    });

    std::vector<ChatMessage> messages = {{"user", "hello"}};
    (void)engine.generateChat(messages, model_name, {});

    EXPECT_TRUE(saw_callback);
    ASSERT_TRUE(captured.has_value());
    EXPECT_EQ(captured->token_count, 2u);
    EXPECT_NEAR(captured->ttft_ms, 100.0, 0.5);
    EXPECT_NEAR(captured->tokens_per_second, 2.0 / 0.3, 0.5);

    InferenceEngine::setTokenMetricsHookForTest({});
    InferenceEngine::setTokenMetricsClockForTest({});
}



// T177: ChatML template rendering tests
TEST(ChatTemplateTest, BuildsChatMLPromptWithSingleMessage) {
    std::vector<ChatMessage> messages = {{"user", "Hello"}};
    std::string result = buildChatMLPrompt(messages);

    EXPECT_TRUE(result.find("<|im_start|>user\nHello<|im_end|>") != std::string::npos);
    EXPECT_TRUE(result.find("<|im_start|>assistant\n") != std::string::npos);
}

TEST(ChatTemplateTest, BuildsChatMLPromptWithMultipleMessages) {
    std::vector<ChatMessage> messages = {
        {"system", "You are a helpful assistant."},
        {"user", "What is 2+2?"},
        {"assistant", "4"},
        {"user", "What is 3+3?"}
    };
    std::string result = buildChatMLPrompt(messages);

    EXPECT_TRUE(result.find("<|im_start|>system\nYou are a helpful assistant.<|im_end|>") != std::string::npos);
    EXPECT_TRUE(result.find("<|im_start|>user\nWhat is 2+2?<|im_end|>") != std::string::npos);
    EXPECT_TRUE(result.find("<|im_start|>assistant\n4<|im_end|>") != std::string::npos);
    EXPECT_TRUE(result.find("<|im_start|>user\nWhat is 3+3?<|im_end|>") != std::string::npos);
    // Ends with assistant prompt (the final segment of the prompt)
    EXPECT_TRUE(result.find("<|im_start|>assistant\n") != std::string::npos);
    // Verify it ends with the assistant prompt marker
    const std::string assistant_marker = "<|im_start|>assistant\n";
    EXPECT_EQ(result.substr(result.size() - assistant_marker.size()), assistant_marker);
}

TEST(ChatTemplateTest, BuildsChatMLPromptWithEmptyContent) {
    std::vector<ChatMessage> messages = {{"user", ""}};
    std::string result = buildChatMLPrompt(messages);

    EXPECT_TRUE(result.find("<|im_start|>user\n<|im_end|>") != std::string::npos);
}

TEST(ChatTemplateTest, BuildsChatMLPromptWithMultilineContent) {
    std::vector<ChatMessage> messages = {{"user", "Line1\nLine2\nLine3"}};
    std::string result = buildChatMLPrompt(messages);

    EXPECT_TRUE(result.find("<|im_start|>user\nLine1\nLine2\nLine3<|im_end|>") != std::string::npos);
}

TEST(ChatTemplateTest, BuildsChatMLPromptPreservesMessageOrder) {
    std::vector<ChatMessage> messages = {
        {"user", "First"},
        {"assistant", "Second"},
        {"user", "Third"}
    };
    std::string result = buildChatMLPrompt(messages);

    size_t pos_first = result.find("First");
    size_t pos_second = result.find("Second");
    size_t pos_third = result.find("Third");

    EXPECT_LT(pos_first, pos_second);
    EXPECT_LT(pos_second, pos_third);
}

// =============================================================================
// T144: 指数バックオフリトライテスト
// =============================================================================

TEST(RetryTest, SucceedsOnFirstAttempt) {
    RetryConfig config;
    config.max_retries = 4;
    config.initial_delay = std::chrono::milliseconds(1);

    int attempt_count = 0;
    auto result = with_retry(
        [&]() {
            ++attempt_count;
            return 42;
        },
        config
    );

    EXPECT_EQ(result, 42);
    EXPECT_EQ(attempt_count, 1);
}

TEST(RetryTest, RetriesOnFailureThenSucceeds) {
    RetryConfig config;
    config.max_retries = 4;
    config.initial_delay = std::chrono::milliseconds(1);

    int attempt_count = 0;
    auto result = with_retry(
        [&]() -> int {
            ++attempt_count;
            if (attempt_count < 3) {
                throw std::runtime_error("temporary error");
            }
            return 42;
        },
        config
    );

    EXPECT_EQ(result, 42);
    EXPECT_EQ(attempt_count, 3);  // Failed twice, succeeded on third
}

TEST(RetryTest, ThrowsAfterMaxRetries) {
    RetryConfig config;
    config.max_retries = 2;
    config.initial_delay = std::chrono::milliseconds(1);

    int attempt_count = 0;
    EXPECT_THROW({
        with_retry(
            [&]() -> int {
                ++attempt_count;
                throw std::runtime_error("persistent error");
            },
            config
        );
    }, std::runtime_error);

    EXPECT_EQ(attempt_count, 3);  // Initial + 2 retries
}

TEST(RetryTest, CallsOnCrashCallback) {
    RetryConfig config;
    config.max_retries = 3;
    config.initial_delay = std::chrono::milliseconds(1);

    int attempt_count = 0;
    int crash_callback_count = 0;

    EXPECT_THROW({
        with_retry(
            [&]() -> int {
                ++attempt_count;
                throw std::runtime_error("crash");
            },
            config,
            [&]() { ++crash_callback_count; }
        );
    }, std::runtime_error);

    EXPECT_EQ(attempt_count, 4);  // Initial + 3 retries
    EXPECT_EQ(crash_callback_count, 3);  // Called after each failed retry (except last)
}

TEST(RetryTest, ExponentialBackoffDelays) {
    RetryConfig config;
    config.max_retries = 3;
    config.initial_delay = std::chrono::milliseconds(10);
    config.max_total = std::chrono::milliseconds(1000);

    int attempt_count = 0;
    auto start = std::chrono::steady_clock::now();

    // This test will fail after max_retries, but we measure the delay
    EXPECT_THROW({
        with_retry(
            [&]() -> int {
                ++attempt_count;
                throw std::runtime_error("fail");
            },
            config
        );
    }, std::runtime_error);

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start);

    // Minimum delay: 10ms + 20ms + 40ms = 70ms
    // Allow some margin for execution time
    EXPECT_GE(elapsed.count(), 60);
    EXPECT_EQ(attempt_count, 4);
}

TEST(RetryTest, StopsWhenMaxTotalExceeded) {
    RetryConfig config;
    config.max_retries = 100;  // High number
    config.initial_delay = std::chrono::milliseconds(50);
    config.max_total = std::chrono::milliseconds(100);  // Low total limit

    int attempt_count = 0;
    auto start = std::chrono::steady_clock::now();

    EXPECT_THROW({
        with_retry(
            [&]() -> int {
                ++attempt_count;
                throw std::runtime_error("fail");
            },
            config
        );
    }, std::runtime_error);

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start);

    // Should stop due to max_total limit, not max_retries
    EXPECT_LT(attempt_count, 10);
    // Total time should be less than 2x the max_total
    EXPECT_LT(elapsed.count(), 300);
}

// =============================================================================
// T181, T188: クラッシュ後503即時返却テスト
// =============================================================================

TEST(ServiceUnavailableTest, ThrowsCorrectException) {
    ServiceUnavailableError error("test message");
    EXPECT_STREQ(error.what(), "test message");
}



// =============================================================================
// T182, T189: トークン間タイムアウトテスト
// =============================================================================

TEST(TokenTimeoutTest, TokenTimeoutErrorHasCorrectMessage) {
    TokenTimeoutError error("inter-token timeout: 5000ms");
    EXPECT_STREQ(error.what(), "inter-token timeout: 5000ms");
}

TEST(TokenTimeoutTest, TokenTimeoutErrorInheritsFromRuntimeError) {
    TokenTimeoutError error("test");
    // Verify it can be caught as std::runtime_error
    try {
        throw error;
    } catch (const std::runtime_error& e) {
        EXPECT_STREQ(e.what(), "test");
    }
}

TEST(TokenTimeoutTest, AbortCallbackInInferenceParamsDefaultsToNull) {
    InferenceParams params;
    EXPECT_EQ(params.abort_callback, nullptr);
    EXPECT_EQ(params.abort_callback_ctx, nullptr);
}
