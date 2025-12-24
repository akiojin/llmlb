#include "core/whisper_manager.h"

#include <gtest/gtest.h>

namespace {

TEST(WhisperManagerTest, FlashAttentionIsDisabledByDefault) {
    EXPECT_FALSE(llm_node::WhisperManager::shouldUseFlashAttention());
}

}  // namespace
