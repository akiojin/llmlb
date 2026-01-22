#include <gtest/gtest.h>

#include <algorithm>
#include <filesystem>
#include <string>
#include <vector>

#include "core/llama_manager.h"
#include "core/text_manager.h"
#include "models/model_descriptor.h"

namespace fs = std::filesystem;

namespace {

class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("text-manager-XXXXXX");
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

bool contains(const std::vector<std::string>& values, const std::string& needle) {
    return std::find(values.begin(), values.end(), needle) != values.end();
}

}  // namespace

using xllm::LlamaManager;
using xllm::ModelDescriptor;
using xllm::TextManager;

TEST(TextManagerTest, RegistersLlamaRuntime) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    TextManager manager(llama, tmp.path.string());

    auto runtimes = manager.getRegisteredRuntimes();
    EXPECT_TRUE(contains(runtimes, "llama_cpp"));
}

TEST(TextManagerTest, ResolvesTextEngineForLlama) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    TextManager manager(llama, tmp.path.string());

    ModelDescriptor desc;
    desc.runtime = "llama_cpp";
    desc.format = "gguf";

    std::string error;
    auto* engine = manager.resolve(desc, "text", &error);
    EXPECT_NE(engine, nullptr);
    EXPECT_TRUE(error.empty());
    EXPECT_EQ(manager.engineIdFor(engine), "llama_cpp");
}

TEST(TextManagerTest, SupportsArchitectureNormalization) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    TextManager manager(llama, tmp.path.string());

    EXPECT_TRUE(manager.supportsArchitecture("llama_cpp", {"Llama-3"}));
    EXPECT_FALSE(manager.supportsArchitecture("llama_cpp", {"gpt-oss"}));
    EXPECT_FALSE(manager.supportsArchitecture("missing", {"llama"}));
}

TEST(TextManagerTest, ReturnsErrorWhenRegistryMissing) {
    TempDir tmp;
    LlamaManager llama(tmp.path.string());
    TextManager manager(llama, tmp.path.string());

    manager.setEngineRegistryForTest(nullptr);

    ModelDescriptor desc;
    desc.runtime = "llama_cpp";

    std::string error;
    EXPECT_EQ(manager.resolve(desc, "text", &error), nullptr);
    EXPECT_EQ(error, "TextManager not initialized");
    EXPECT_TRUE(manager.getRegisteredRuntimes().empty());
}
