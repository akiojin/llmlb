#include <gtest/gtest.h>

#include <algorithm>
#include <filesystem>

#include "core/llama_manager.h"
#include "core/text_manager.h"

namespace fs = std::filesystem;

class TempModelDir {
public:
    TempModelDir() {
        auto base = fs::temp_directory_path();
        path = base / ("text-manager-" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
        fs::create_directories(path);
    }
    ~TempModelDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }
    fs::path path;
};

TEST(TextManagerTest, RegistersExpectedRuntimes) {
    TempModelDir tmp;
    allm::LlamaManager llama(tmp.path.string());
    allm::TextManager text(llama, tmp.path.string());

    auto runtimes = text.getRegisteredRuntimes();
    auto has_runtime = [&](const std::string& value) {
        return std::find(runtimes.begin(), runtimes.end(), value) != runtimes.end();
    };

    EXPECT_TRUE(has_runtime("llama_cpp"));
#ifdef ALLM_WITH_SAFETENSORS
    EXPECT_TRUE(has_runtime("safetensors_cpp"));
#endif
}

TEST(TextManagerTest, SupportsArchitectureFamilies) {
    TempModelDir tmp;
    allm::LlamaManager llama(tmp.path.string());
    allm::TextManager text(llama, tmp.path.string());

    EXPECT_TRUE(text.supportsArchitecture("llama_cpp", {"llama"}));
    EXPECT_TRUE(text.supportsArchitecture("llama_cpp", {"mistral"}));
    EXPECT_FALSE(text.supportsArchitecture("llama_cpp", {"gptoss"}));

#ifdef ALLM_WITH_SAFETENSORS
    EXPECT_TRUE(text.supportsArchitecture("safetensors_cpp", {"gptoss"}));
    EXPECT_TRUE(text.supportsArchitecture("safetensors_cpp", {"nemotron"}));
    EXPECT_TRUE(text.supportsArchitecture("safetensors_cpp", {"qwen"}));
    EXPECT_TRUE(text.supportsArchitecture("safetensors_cpp", {"glm"}));
#endif
}
