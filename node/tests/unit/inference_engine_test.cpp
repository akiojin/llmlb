#include <gtest/gtest.h>
#include <cstdlib>
#include <filesystem>
#include <fstream>

#include "core/inference_engine.h"
#include "core/llama_manager.h"
#include "models/model_descriptor.h"
#include "models/model_storage.h"

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
