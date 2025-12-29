#include <gtest/gtest.h>

#include <cstdlib>
#include <filesystem>
#include <fstream>

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
