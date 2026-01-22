#include "core/whisper_manager.h"

#include <gtest/gtest.h>
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

TEST(WhisperManagerTest, FlashAttentionIsDisabledByDefault) {
    EnvGuard guard({"XLLM_WHISPER_FLASH_ATTN"});
    unsetenv("XLLM_WHISPER_FLASH_ATTN");
    EXPECT_FALSE(xllm::WhisperManager::shouldUseFlashAttention());
}

TEST(WhisperManagerTest, FlashAttentionEnabledWhenEnvSet) {
    EnvGuard guard({"XLLM_WHISPER_FLASH_ATTN"});
    setenv("XLLM_WHISPER_FLASH_ATTN", "1", 1);
    EXPECT_TRUE(xllm::WhisperManager::shouldUseFlashAttention());

    setenv("XLLM_WHISPER_FLASH_ATTN", "0", 1);
    EXPECT_FALSE(xllm::WhisperManager::shouldUseFlashAttention());
}

TEST(WhisperManagerTest, InitialStateIsEmpty) {
    xllm::WhisperManager manager("/tmp");
    EXPECT_EQ(manager.loadedCount(), 0u);
    EXPECT_TRUE(manager.getLoadedModels().empty());
    EXPECT_FALSE(manager.getLastAccessTime("missing").has_value());
}

TEST(WhisperManagerTest, TranscribeRejectsEmptyAudio) {
    xllm::WhisperManager manager("/tmp");
    xllm::TranscriptionParams params;
    auto result = manager.transcribe("missing.bin", {}, 16000, params);
    EXPECT_FALSE(result.success);
    EXPECT_EQ(result.error, "Empty audio data");
}

TEST(WhisperManagerTest, TranscribeRejectsInvalidSampleRate) {
    xllm::WhisperManager manager("/tmp");
    std::vector<float> audio_data = {0.0f, 0.1f};
    auto result = manager.transcribe("missing.bin", audio_data, 8000);
    EXPECT_FALSE(result.success);
    EXPECT_NE(result.error.find("Sample rate must be 16000 Hz"), std::string::npos);
}

TEST(WhisperManagerTest, TranscribeRejectsUnloadedModel) {
    xllm::WhisperManager manager("/tmp");
    std::vector<float> audio_data = {0.0f};
    auto result = manager.transcribe("missing.bin", audio_data, 16000);
    EXPECT_FALSE(result.success);
    EXPECT_NE(result.error.find("Model not loaded:"), std::string::npos);
}

}  // namespace
