#include <gtest/gtest.h>
#include <cstdlib>
#include <filesystem>
#include <fstream>

#include "core/inference_engine.h"
#include "core/llama_manager.h"
#include "core/engine_registry.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "api/http_server.h"
#include "models/model_registry.h"
#include "models/model_descriptor.h"
#include "models/model_storage.h"
#include "system/resource_monitor.h"

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
        result.code = EngineErrorCode::kOk;
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
        result.code = EngineErrorCode::kOk;
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
    EXPECT_EQ(result.code, EngineErrorCode::kUnavailable);
    EXPECT_NE(result.error_message.find("not initialized"), std::string::npos);
}

TEST(InferenceEngineTest, LoadModelReturnsNotFoundWhenMissingModel) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    ModelStorage storage(tmp.path.string());
    InferenceEngine engine(llama, storage);

    auto result = engine.loadModel("missing/model");
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.code, EngineErrorCode::kNotFound);
    EXPECT_NE(result.error_message.find("Model not found"), std::string::npos);
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
    EXPECT_EQ(result.code, EngineErrorCode::kUnsupported);
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
    EXPECT_EQ(result.code, EngineErrorCode::kUnsupported);
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
    EXPECT_EQ(result.code, EngineErrorCode::kResourceExhausted);
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
    EXPECT_EQ(result.code, EngineErrorCode::kResourceExhausted);
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

TEST(InferenceEngineTest, GptOssRequiresDirectmlArtifactToBeSupported) {
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

    std::ofstream(model_dir / "model.directml.bin") << "cache";
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
