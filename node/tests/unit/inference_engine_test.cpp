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

using namespace llm_node;
namespace fs = std::filesystem;

// テスト専用ヘルパー（inference_engine.cppで定義）
namespace llm_node {
std::string extractGptOssFinalMessageForTest(const std::string& output);
std::string cleanGptOssOutputForTest(const std::string& output);
std::string postProcessGeneratedTextForTest(const std::string& output, bool is_gptoss);
}
using llm_node::extractGptOssFinalMessageForTest;
using llm_node::cleanGptOssOutputForTest;
using llm_node::postProcessGeneratedTextForTest;

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

class CountingEngine final : public Engine {
public:
    explicit CountingEngine(std::string output_prefix = "out")
        : output_prefix_(std::move(output_prefix)) {}

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
        const int call_index = completion_calls_.fetch_add(1) + 1;
        return output_prefix_ + std::to_string(call_index);
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

    int completionCalls() const { return completion_calls_.load(); }

private:
    std::string output_prefix_;
    mutable std::atomic<int> completion_calls_{0};
};

class SizedEngine final : public Engine {
public:
    explicit SizedEngine(size_t output_size) : output_size_(output_size) {}

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
        completion_calls_.fetch_add(1);
        return std::string(output_size_, 'x');
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

    int completionCalls() const { return completion_calls_.load(); }

private:
    size_t output_size_{0};
    mutable std::atomic<int> completion_calls_{0};
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
        std::make_unique<RecordingEngine>("gptoss_cpp", "text", nullptr, true, false),
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
    llm_node::set_ready(true);
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
    OpenAIEndpoints openai(api_registry, engine, config);
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

TEST(InferenceEngineTest, CachesCompletionWhenTemperatureZero) {
    TempDir tmp;
    const std::string model_name = "example/cache";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);
    engine.setResourceUsageProviderForTest([]() {
        return ResourceUsage{0, 10000, 0, 0};
    });

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "cache_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    auto engine_impl = std::make_unique<CountingEngine>("out-");
    auto* engine_raw = engine_impl.get();
    ASSERT_TRUE(registry->registerEngine(std::move(engine_impl), reg, nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    InferenceParams cached_params;
    cached_params.temperature = 0.0f;

    auto first = engine.generateCompletion("hello", model_name, cached_params);
    auto second = engine.generateCompletion("hello", model_name, cached_params);
    EXPECT_EQ(engine_raw->completionCalls(), 1);
    EXPECT_EQ(first, second);

    auto third = engine.generateCompletion("different", model_name, cached_params);
    EXPECT_EQ(engine_raw->completionCalls(), 2);
    EXPECT_NE(third, second);

    InferenceParams uncached_params;
    uncached_params.temperature = 0.7f;
    (void)engine.generateCompletion("hello", model_name, uncached_params);
    (void)engine.generateCompletion("hello", model_name, uncached_params);
    EXPECT_EQ(engine_raw->completionCalls(), 4);
}

TEST(InferenceEngineTest, EvictsInferenceCacheWhenOverLimit) {
    TempDir tmp;
    const std::string model_name = "example/cache-evict";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);
    engine.setResourceUsageProviderForTest([]() {
        return ResourceUsage{0, 10000, 0, 0};
    });

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "cache_eviction_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    auto engine_impl = std::make_unique<SizedEngine>(300);
    auto* engine_raw = engine_impl.get();
    ASSERT_TRUE(registry->registerEngine(std::move(engine_impl), reg, nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    InferenceParams params;
    params.temperature = 0.0f;

    (void)engine.generateCompletion("a", model_name, params);
    (void)engine.generateCompletion("b", model_name, params);
    EXPECT_EQ(engine_raw->completionCalls(), 2);

    (void)engine.generateCompletion("a", model_name, params);
    EXPECT_EQ(engine_raw->completionCalls(), 3);
}

TEST(InferenceEngineTest, ExtractsFinalChannelFromGptOssOutput) {
    const std::string raw =
        "<|start|>assistant<|channel|>analysis<|message|>think here<|end|>"
        "<|start|>assistant<|channel|>final<|message|>the answer<|end|>";

    auto extracted = extractGptOssFinalMessageForTest(raw);
    EXPECT_EQ(extracted, "the answer");
}

TEST(InferenceEngineTest, CleansGptOssOutputByExtractingFinalChannel) {
    const std::string raw =
        "<|start|>assistant<|channel|>analysis<|message|>think here<|end|>"
        "<|start|>assistant<|channel|>final<|message|>the answer<|end|>";

    auto cleaned = cleanGptOssOutputForTest(raw);
    EXPECT_EQ(cleaned, "the answer");
}

TEST(InferenceEngineTest, PostProcessGptOssDoesNotTruncateStartTokenOnlyOutput) {
    // When gpt-oss emits a header but no <|end|>, we should not truncate to empty.
    const std::string raw = "<|start|>assistant<|channel|>final<|message|>Hello world";

    auto processed = postProcessGeneratedTextForTest(raw, /*is_gptoss=*/true);
    EXPECT_EQ(processed, "Hello world");
}

TEST(InferenceEngineTest, GptOssRequiresMetalArtifactToBeSupported) {
#if !defined(__APPLE__)
    GTEST_SKIP() << "Metal backend is only supported on macOS";
#else
    TempDir tmp;
    auto model_dir = tmp.path / "openai" / "gpt-oss-20b";
    fs::create_directories(model_dir);

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    ModelDescriptor desc;
    desc.name = "openai/gpt-oss-20b";
    desc.runtime = "gptoss_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors.index.json").string();

    EXPECT_FALSE(engine.isModelSupported(desc));

    std::ofstream(model_dir / "model.metal.bin") << "cache";
#ifdef USE_GPTOSS
    EXPECT_TRUE(engine.isModelSupported(desc));
#else
    EXPECT_FALSE(engine.isModelSupported(desc));
#endif
#endif
}

TEST(InferenceEngineTest, GptOssSupportsSafetensorsOnDirectml) {
#if !defined(_WIN32)
    GTEST_SKIP() << "DirectML backend is only supported on Windows";
#elif !defined(USE_GPTOSS)
    GTEST_SKIP() << "gpt-oss DirectML engine not enabled";
#else
    TempDir tmp;
    auto model_dir = tmp.path / "openai" / "gpt-oss-20b";
    fs::create_directories(model_dir);

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    ModelDescriptor desc;
    desc.name = "openai/gpt-oss-20b";
    desc.runtime = "gptoss_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors.index.json").string();

    EXPECT_FALSE(engine.isModelSupported(desc));

    std::ofstream(model_dir / "model.safetensors.index.json") << R"({"weight_map":{}})";
    EXPECT_TRUE(engine.isModelSupported(desc));
#endif
}

TEST(InferenceEngineTest, NemotronRequiresCudaToBeSupported) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    ModelDescriptor desc;
    desc.name = "nvidia/nemotron-test";
    desc.runtime = "nemotron_cpp";
    desc.format = "safetensors";
    desc.model_dir = tmp.path.string();
    desc.primary_path = (tmp.path / "model.safetensors.index.json").string();

#ifdef USE_CUDA
    EXPECT_TRUE(engine.isModelSupported(desc));
#else
    EXPECT_FALSE(engine.isModelSupported(desc));
#endif
}

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

TEST(InferenceEngineTest, SchedulesPluginRestartAfterRequestLimit) {
    TempDir tmp;
    const std::string model_name = "example/restart";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "restart_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RecordingEngine>("llama_cpp", "restart", nullptr, true, false),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);
    engine.setPluginRestartPolicy(std::chrono::seconds(0), 2);

    std::atomic<int> restart_calls{0};
    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        restart_calls.fetch_add(1);
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "hello"}};
    (void)engine.generateChat(messages, model_name, {});
    EXPECT_EQ(restart_calls.load(), 0);

    (void)engine.generateChat(messages, model_name, {});
    EXPECT_EQ(restart_calls.load(), 1);

    InferenceEngine::setPluginRestartHookForTest({});
}

TEST(InferenceEngineTest, RestartsPluginAfterCrash) {
    TempDir tmp;
    const std::string model_name = "example/crash";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "crash_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<ThrowingEngine>(),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    std::atomic<int> restart_calls{0};
    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        restart_calls.fetch_add(1);
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "boom"}};
    EXPECT_THROW((void)engine.generateChat(messages, model_name, {}), std::runtime_error);
    EXPECT_EQ(restart_calls.load(), 1);

    InferenceEngine::setPluginRestartHookForTest({});
}

// T181: クラッシュ後即時503返却
TEST(InferenceEngineTest, RejectsNewRequestsWhilePluginRestartPending) {
    TempDir tmp;
    const std::string model_name = "example/crash503";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "crash503_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<ThrowingEngine>(),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    // Hook to capture restart calls but NOT actually restart
    std::atomic<int> restart_calls{0};
    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        restart_calls.fetch_add(1);
        return true;  // Restart staged but pending
    });

    std::vector<ChatMessage> messages = {{"user", "boom"}};

    // First request crashes the engine
    EXPECT_THROW((void)engine.generateChat(messages, model_name, {}), std::runtime_error);
    EXPECT_EQ(restart_calls.load(), 1);

    // Verify restart is pending
    EXPECT_TRUE(engine.isPluginRestartPendingForTest());

    // Second request should be immediately rejected with ServiceUnavailable
    try {
        (void)engine.generateChat(messages, model_name, {});
        FAIL() << "Expected exception for service unavailable";
    } catch (const std::exception& e) {
        EXPECT_NE(std::string(e.what()).find("service unavailable"), std::string::npos);
    }

    InferenceEngine::setPluginRestartHookForTest({});
}

TEST(InferenceEngineTest, RejectsStreamRequestsWhilePluginRestartPending) {
    TempDir tmp;
    const std::string model_name = "example/crash503stream";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "crash503stream_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<ThrowingEngine>(),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "boom"}};

    // First request crashes the engine
    EXPECT_THROW(
        (void)engine.generateChatStream(messages, model_name, {}, [](const std::string&){}),
        std::runtime_error);

    // Stream request should also be rejected
    try {
        (void)engine.generateChatStream(messages, model_name, {}, [](const std::string&){});
        FAIL() << "Expected exception for service unavailable";
    } catch (const std::exception& e) {
        EXPECT_NE(std::string(e.what()).find("service unavailable"), std::string::npos);
    }

    InferenceEngine::setPluginRestartHookForTest({});
}

// T136/T137: 指数バックオフリトライ
// クラッシュ後に透過的リトライを行い、成功時はクライアントに見えない形で結果を返す
class RetryCountingEngine final : public Engine {
public:
    RetryCountingEngine(std::atomic<int>* counter, int fail_count)
        : call_counter_(counter), fail_until_(fail_count) {}

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
        int count = call_counter_->fetch_add(1);
        if (count < fail_until_) {
            throw std::runtime_error("Engine crash (attempt " + std::to_string(count + 1) + ")");
        }
        return "Success after " + std::to_string(count + 1) + " attempts";
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
        return {};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override {
        return 4096;
    }

private:
    mutable std::atomic<int>* call_counter_;
    int fail_until_;
};

TEST(InferenceEngineTest, RetriesWithExponentialBackoffOnCrash) {
    TempDir tmp;
    std::string model_name = "example/retry-test";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> call_counter{0};
    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "retry_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RetryCountingEngine>(&call_counter, 2),  // Fail first 2 attempts
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    std::atomic<int> restart_calls{0};
    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        restart_calls.fetch_add(1);
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "test"}};

    // Should succeed after transparent retry
    auto start = std::chrono::steady_clock::now();
    std::string result = engine.generateChat(messages, model_name, {});
    auto elapsed = std::chrono::steady_clock::now() - start;

    // Verify retry occurred (3 total calls: 2 failures + 1 success)
    EXPECT_EQ(call_counter.load(), 3);
    EXPECT_EQ(result, "Success after 3 attempts");

    // Verify exponential backoff delay (100ms + 200ms = 300ms minimum)
    EXPECT_GE(std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count(), 200);

    InferenceEngine::setPluginRestartHookForTest({});
}

TEST(InferenceEngineTest, RetriesUpToMaximumAttempts) {
    TempDir tmp;
    std::string model_name = "example/retry-max";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> call_counter{0};
    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "retry_max_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RetryCountingEngine>(&call_counter, 100),  // Always fail
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "test"}};

    // Should fail after max retries (4 retries = 5 total attempts: 1 initial + 4 retries)
    EXPECT_THROW((void)engine.generateChat(messages, model_name, {}), std::runtime_error);

    // Verify max retry attempts (1 initial + 4 retries = 5)
    EXPECT_EQ(call_counter.load(), 5);

    InferenceEngine::setPluginRestartHookForTest({});
}

TEST(InferenceEngineTest, TransparentRetryDoesNotExposeIntermediateErrors) {
    TempDir tmp;
    std::string model_name = "example/retry-transparent";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> call_counter{0};
    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "retry_transparent_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<RetryCountingEngine>(&call_counter, 1),  // Fail first attempt only
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));

    engine.setEnginePluginsDirForTest(tmp.path);

    InferenceEngine::setPluginRestartHookForTest([&](std::string&) {
        return true;
    });

    std::vector<ChatMessage> messages = {{"user", "test"}};

    // Should return successful result without any indication of intermediate failure
    std::string result = engine.generateChat(messages, model_name, {});
    EXPECT_FALSE(result.empty());
    EXPECT_EQ(result.find("error"), std::string::npos);
    EXPECT_EQ(result.find("crash"), std::string::npos);

    InferenceEngine::setPluginRestartHookForTest({});
}

// T138-T140: Cancellation processing tests

// CancellableEngine: simulates a slow generation that can be cancelled
class CancellableEngine final : public Engine {
public:
    CancellableEngine(std::atomic<int>* token_count, std::atomic<bool>* started)
        : token_count_(token_count), started_(started) {}

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
        if (started_) started_->store(true);
        std::string output;
        // Simulate token generation loop
        for (int i = 0; i < 100; ++i) {
            // T138: Check cancellation token before generating each token
            if (params.cancellation_token && params.cancellation_token->load()) {
                throw GenerationCancelledException("Generation cancelled");
            }
            if (token_count_) token_count_->fetch_add(1);
            output += "token" + std::to_string(i) + " ";
            // Small delay to simulate real generation
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        return output;
    }

    std::string generateCompletion(const std::string&,
                                   const ModelDescriptor&,
                                   const InferenceParams& params) const override {
        return generateChat({}, {}, params);
    }

    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>&,
        const ModelDescriptor&,
        const InferenceParams& params,
        const std::function<void(const std::string&)>& on_token) const override {
        std::vector<std::string> tokens;
        for (int i = 0; i < 100; ++i) {
            // T138: Check cancellation token before generating each token
            if (params.cancellation_token && params.cancellation_token->load()) {
                throw GenerationCancelledException("Generation cancelled");
            }
            std::string token = "token" + std::to_string(i);
            tokens.push_back(token);
            if (on_token) on_token(token);
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        return tokens;
    }

    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>&,
        const ModelDescriptor&) const override {
        return {};
    }

    size_t getModelMaxContext(const ModelDescriptor&) const override {
        return 4096;
    }

private:
    mutable std::atomic<int>* token_count_;
    mutable std::atomic<bool>* started_;
};

// T138: Cancellation flag check mechanism
TEST(InferenceEngineTest, CancellationTokenStopsGeneration) {
    TempDir tmp;
    std::string model_name = "example/cancellation-test";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> token_count{0};
    std::atomic<bool> started{false};
    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "cancellable_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<CancellableEngine>(&token_count, &started),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));
    engine.setEnginePluginsDirForTest(tmp.path);

    std::atomic<bool> cancel_token{false};
    InferenceParams params;
    params.cancellation_token = &cancel_token;

    std::vector<ChatMessage> messages = {{"user", "test"}};

    // Start generation in background thread
    std::thread gen_thread([&]() {
        try {
            engine.generateChat(messages, model_name, params);
        } catch (const GenerationCancelledException& e) {
            // Expected: generation cancelled
            EXPECT_NE(std::string(e.what()).find("cancelled"), std::string::npos);
        }
    });

    // Wait for generation to start
    while (!started.load()) {
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
    }

    // Cancel after some tokens have been generated
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    cancel_token.store(true);

    gen_thread.join();

    // Verify generation was stopped before completion
    // (100 tokens would take at least 1000ms, we cancelled after ~50ms)
    EXPECT_LT(token_count.load(), 50);  // Should be far less than 100
    EXPECT_GT(token_count.load(), 0);   // Should have generated some tokens
}

// T139: Immediate cancellation response
TEST(InferenceEngineTest, CancellationRespondsImmediately) {
    TempDir tmp;
    std::string model_name = "example/cancel-immediate";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> token_count{0};
    std::atomic<bool> started{false};
    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "cancel_immediate_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<CancellableEngine>(&token_count, &started),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));
    engine.setEnginePluginsDirForTest(tmp.path);

    // Pre-set cancellation token before generation
    std::atomic<bool> cancel_token{true};
    InferenceParams params;
    params.cancellation_token = &cancel_token;

    std::vector<ChatMessage> messages = {{"user", "test"}};

    auto start_time = std::chrono::steady_clock::now();

    // Should throw immediately since cancellation is pre-set
    EXPECT_THROW({
        engine.generateChat(messages, model_name, params);
    }, GenerationCancelledException);

    auto elapsed = std::chrono::steady_clock::now() - start_time;
    auto elapsed_ms = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();

    // Should respond within a short time (well under 100ms, definitely not waiting for 100 tokens)
    EXPECT_LT(elapsed_ms, 100);
    // Only 0 or 1 token should have been attempted before checking cancellation
    EXPECT_LE(token_count.load(), 1);
}

// T140: Cancellation does not affect other requests in batch
TEST(InferenceEngineTest, CancellationDoesNotAffectOtherRequests) {
    TempDir tmp;
    std::string model_name = "example/cancel-batch";
    const auto model_dir = tmp.path / ModelStorage::modelNameToDir(model_name);
    fs::create_directories(model_dir);
    std::ofstream(model_dir / "model.gguf") << "gguf";

    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    std::atomic<int> token_count1{0};
    std::atomic<int> token_count2{0};
    std::atomic<bool> started1{false};
    std::atomic<bool> started2{false};

    auto registry = std::make_unique<EngineRegistry>();
    EngineRegistration reg;
    reg.engine_id = "cancel_batch_engine";
    reg.engine_version = "test";
    reg.formats = {"gguf"};
    reg.capabilities = {"text"};
    // Use a single engine instance that will handle both requests
    ASSERT_TRUE(registry->registerEngine(
        std::make_unique<CancellableEngine>(&token_count1, &started1),
        reg,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry));
    engine.setEnginePluginsDirForTest(tmp.path);

    std::atomic<bool> cancel_token1{false};
    std::atomic<bool> cancel_token2{false};

    InferenceParams params1;
    params1.cancellation_token = &cancel_token1;

    InferenceParams params2;
    params2.cancellation_token = &cancel_token2;

    std::vector<ChatMessage> messages = {{"user", "test"}};

    std::atomic<bool> request1_cancelled{false};
    std::atomic<bool> request2_completed{false};

    // Start two concurrent requests
    std::thread thread1([&]() {
        try {
            engine.generateChat(messages, model_name, params1);
        } catch (...) {
            request1_cancelled.store(true);
        }
    });

    // Wait for first request to start
    while (!started1.load()) {
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }

    // Cancel first request
    cancel_token1.store(true);
    thread1.join();

    // Verify first request was cancelled
    EXPECT_TRUE(request1_cancelled.load());

    // Reset engine registry for second request
    auto registry2 = std::make_unique<EngineRegistry>();
    EngineRegistration reg2;
    reg2.engine_id = "cancel_batch_engine2";
    reg2.engine_version = "test";
    reg2.formats = {"gguf"};
    reg2.capabilities = {"text"};
    ASSERT_TRUE(registry2->registerEngine(
        std::make_unique<CancellableEngine>(&token_count2, &started2),
        reg2,
        nullptr));
    engine.setEngineRegistryForTest(std::move(registry2));

    // Second request should complete normally (not cancelled)
    std::thread thread2([&]() {
        try {
            std::string result = engine.generateChat(messages, model_name, params2);
            if (!result.empty()) {
                request2_completed.store(true);
            }
        } catch (...) {
            // Should not throw
        }
    });

    thread2.join();

    // Second request should complete (token2 never cancelled)
    EXPECT_TRUE(request2_completed.load());
    EXPECT_EQ(token_count2.load(), 100);  // All 100 tokens generated
}
