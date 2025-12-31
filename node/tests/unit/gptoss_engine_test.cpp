#include <gtest/gtest.h>

#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <optional>

#include "core/gptoss_engine.h"
#include "models/model_descriptor.h"

using namespace llm_node;
namespace fs = std::filesystem;

namespace {
class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path();
        for (int i = 0; i < 10; ++i) {
            auto candidate = base / fs::path("gptoss-engine-" + std::to_string(std::rand()));
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

void write_text(const fs::path& path, const std::string& content) {
    std::ofstream ofs(path);
    ofs << content;
}

std::optional<ModelDescriptor> build_gptoss_descriptor_from_env(std::string& error) {
    const char* model_dir_env = std::getenv("LLM_NODE_GPTOSS_TEST_MODEL_DIR");
    if (!model_dir_env || std::string(model_dir_env).empty()) {
        error = "LLM_NODE_GPTOSS_TEST_MODEL_DIR is not set";
        return std::nullopt;
    }

    fs::path model_dir(model_dir_env);
    if (!fs::exists(model_dir)) {
        error = "gpt-oss model directory does not exist";
        return std::nullopt;
    }

    fs::path primary = model_dir / "model.safetensors.index.json";
    if (!fs::exists(primary)) {
        primary = model_dir / "model.safetensors";
    }
    if (!fs::exists(primary)) {
        error = "safetensors index/model file not found in model dir";
        return std::nullopt;
    }

    ModelDescriptor desc;
    desc.name = "openai/gpt-oss-20b";
    desc.runtime = "gptoss_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = primary.string();
    return desc;
}
}  // namespace

TEST(GptOssEngineTest, SafetensorsRequiresMetadataFiles) {
#ifndef USE_GPTOSS
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    TempDir tmp;
    auto model_dir = tmp.path / "openai" / "gpt-oss-20b";
    fs::create_directories(model_dir);
    write_text(model_dir / "model.safetensors.index.json", R"({"weight_map":{}})");

    ModelDescriptor desc;
    desc.name = "openai/gpt-oss-20b";
    desc.runtime = "gptoss_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors.index.json").string();

    GptOssEngine engine;
    auto res = engine.loadModel(desc);
    EXPECT_FALSE(res.success);
    EXPECT_NE(res.error_message.find("config.json"), std::string::npos);

    write_text(model_dir / "config.json", "{}");
    res = engine.loadModel(desc);
    EXPECT_FALSE(res.success);
    EXPECT_NE(res.error_message.find("tokenizer.json"), std::string::npos);
#endif
}

TEST(GptOssEngineTest, SafetensorsIndexRequiresAllShards) {
#ifndef USE_GPTOSS
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    TempDir tmp;
    auto model_dir = tmp.path / "openai" / "gpt-oss-20b";
    fs::create_directories(model_dir);
    write_text(model_dir / "config.json", "{}");
    write_text(model_dir / "tokenizer.json", "{}");
    write_text(
        model_dir / "model.safetensors.index.json",
        R"({"weight_map":{"layer.0.weight":"model-00001.safetensors"}})");

    ModelDescriptor desc;
    desc.name = "openai/gpt-oss-20b";
    desc.runtime = "gptoss_cpp";
    desc.format = "safetensors";
    desc.model_dir = model_dir.string();
    desc.primary_path = (model_dir / "model.safetensors.index.json").string();

    GptOssEngine engine;
    auto res = engine.loadModel(desc);
    EXPECT_FALSE(res.success);
    EXPECT_NE(res.error_message.find("missing safetensors shard"), std::string::npos);
#endif
}

TEST(GptOssEngineTest, GeneratesTextWhenGpuArtifactsPresent) {
#ifndef USE_GPTOSS
    GTEST_SKIP() << "USE_GPTOSS not enabled";
#else
    std::string error;
    auto desc_opt = build_gptoss_descriptor_from_env(error);
    if (!desc_opt) {
        GTEST_SKIP() << error;
    }
    const auto& desc = *desc_opt;

#if defined(_WIN32)
    if (!fs::exists(fs::path(desc.model_dir) / "model.directml.bin") &&
        !fs::exists(fs::path(desc.model_dir) / "model.dml.bin")) {
        GTEST_SKIP() << "DirectML artifact not found in model dir";
    }
#elif defined(__APPLE__)
    if (!fs::exists(fs::path(desc.model_dir) / "model.metal.bin") &&
        !fs::exists(fs::path(desc.model_dir) / "metal" / "model.bin") &&
        !fs::exists(fs::path(desc.model_dir) / "model.bin")) {
        GTEST_SKIP() << "Metal artifact not found in model dir";
    }
#else
    GTEST_SKIP() << "gpt-oss GPU inference is supported on macOS/Windows only";
#endif

    GptOssEngine engine;
    auto res = engine.loadModel(desc);
    ASSERT_TRUE(res.success) << res.error_message;

    std::vector<ChatMessage> messages = {{"user", "hello"}};
    InferenceParams params;
    params.max_tokens = 8;
    auto output = engine.generateChat(messages, desc, params);
    EXPECT_FALSE(output.empty());
#endif
}
