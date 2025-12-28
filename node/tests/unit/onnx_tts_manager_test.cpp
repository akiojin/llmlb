#include "core/onnx_tts_manager.h"

#include <gtest/gtest.h>
#include <algorithm>
#include <cstdlib>
#include <string>
#include <unordered_map>
#include <vector>

namespace {

class EnvGuard {
public:
    explicit EnvGuard(const std::vector<std::string>& keys) : keys_(keys) {
        for (const auto& key : keys_) {
            const char* value = std::getenv(key.c_str());
            if (value) {
                saved_[key] = value;
            }
        }
    }
    ~EnvGuard() {
        for (const auto& key : keys_) {
            if (auto it = saved_.find(key); it != saved_.end()) {
                setenv(key.c_str(), it->second.c_str(), 1);
            } else {
                unsetenv(key.c_str());
            }
        }
    }

private:
    std::vector<std::string> keys_;
    std::unordered_map<std::string, std::string> saved_;
};

TEST(OnnxTtsManagerTest, RuntimeAvailabilityMatchesBuildFlag) {
#ifdef USE_ONNX_RUNTIME
    EXPECT_TRUE(llm_node::OnnxTtsManager::isRuntimeAvailable());
#else
    EXPECT_FALSE(llm_node::OnnxTtsManager::isRuntimeAvailable());
#endif
}

TEST(OnnxTtsManagerTest, VibeVoiceModelIsAlwaysLoaded) {
    llm_node::OnnxTtsManager manager("/tmp");
    EXPECT_TRUE(manager.loadModel("vibevoice"));
    EXPECT_TRUE(manager.isLoaded("vibevoice"));
    EXPECT_EQ(manager.loadedCount(), 0u);
}

TEST(OnnxTtsManagerTest, SynthesizeRejectsEmptyText) {
    llm_node::OnnxTtsManager manager("/tmp");
    auto result = manager.synthesize("vibevoice", "", {});
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error, "Empty text input");
}

TEST(OnnxTtsManagerTest, VibeVoiceRequiresRunnerEnv) {
    EnvGuard guard({"LLM_NODE_VIBEVOICE_RUNNER"});
    unsetenv("LLM_NODE_VIBEVOICE_RUNNER");

    llm_node::OnnxTtsManager manager("/tmp");
    auto result = manager.synthesize("vibevoice", "hello", {});
    EXPECT_FALSE(result.success);
#if defined(__APPLE__)
    EXPECT_EQ(result.error, "LLM_NODE_VIBEVOICE_RUNNER environment variable not set");
#else
    EXPECT_EQ(result.error, "VibeVoice is only supported on macOS");
#endif
}

TEST(OnnxTtsManagerTest, SupportedVoicesContainsDefaults) {
    llm_node::OnnxTtsManager manager("/tmp");
    auto voices = manager.getSupportedVoices("vibevoice");
    EXPECT_FALSE(voices.empty());
    EXPECT_NE(std::find(voices.begin(), voices.end(), "nova"), voices.end());
}

}  // namespace
