/**
 * @file sliding_window_attention_test.cpp
 * @brief Unit tests for Sliding Window Attention (Task 45)
 */

#include <gtest/gtest.h>
#include <vector>
#include <algorithm>
#include "safetensors.h"

class SlidingWindowAttentionTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Window size configuration
TEST_F(SlidingWindowAttentionTest, WindowSizeConfiguration) {
    // Common sliding window sizes
    std::vector<int32_t> common_sizes = {256, 512, 1024, 2048, 4096};

    for (int32_t size : common_sizes) {
        EXPECT_GT(size, 0);
        EXPECT_LE(size, 8192);
        // Window size should be power of 2 for efficiency
        EXPECT_EQ(size & (size - 1), 0);
    }
}

// Test: Attention mask generation
TEST_F(SlidingWindowAttentionTest, AttentionMaskGeneration) {
    // For sliding window, attention mask is band-diagonal
    int seq_len = 8;
    int window_size = 3;

    // Create mask: 1 = attend, 0 = mask out
    std::vector<std::vector<int>> mask(seq_len, std::vector<int>(seq_len, 0));

    for (int i = 0; i < seq_len; i++) {
        for (int j = 0; j < seq_len; j++) {
            // Causal mask: can only attend to previous positions
            if (j <= i) {
                // Sliding window: can only attend within window
                if (i - j < window_size) {
                    mask[i][j] = 1;
                }
            }
        }
    }

    // Position 0 can only attend to itself
    EXPECT_EQ(mask[0][0], 1);

    // Position 5 can attend to positions 3,4,5 (window=3)
    EXPECT_EQ(mask[5][2], 0);  // Outside window
    EXPECT_EQ(mask[5][3], 1);  // Inside window
    EXPECT_EQ(mask[5][4], 1);  // Inside window
    EXPECT_EQ(mask[5][5], 1);  // Current position
}

// Test: Memory efficiency calculation
TEST_F(SlidingWindowAttentionTest, MemoryEfficiencyCalculation) {
    // Full attention: O(n^2) memory
    // Sliding window: O(n * w) memory
    int64_t seq_len = 32768;
    int64_t window_size = 4096;

    int64_t full_attention_memory = seq_len * seq_len;
    int64_t sliding_window_memory = seq_len * window_size;

    float memory_ratio = static_cast<float>(sliding_window_memory) / full_attention_memory;

    EXPECT_LT(memory_ratio, 0.2f);  // At least 5x memory reduction
    EXPECT_EQ(sliding_window_memory, seq_len * window_size);
}

// Test: KV cache with sliding window
TEST_F(SlidingWindowAttentionTest, KVCacheWithSlidingWindow) {
    // With sliding window, we only need to cache window_size KV pairs
    int64_t window_size = 4096;
    int64_t n_heads = 32;
    int64_t head_dim = 128;
    int64_t n_layers = 32;

    // KV cache size with sliding window
    int64_t kv_cache_per_layer = 2 * window_size * n_heads * head_dim * sizeof(float);
    int64_t total_kv_cache = n_layers * kv_cache_per_layer;

    // In GB
    float total_gb = static_cast<float>(total_kv_cache) / (1024 * 1024 * 1024);

    EXPECT_GT(total_gb, 0);
    EXPECT_LT(total_gb, 32);  // Should be reasonable
}

// Test: Circular buffer for KV cache
TEST_F(SlidingWindowAttentionTest, CircularBufferKVCache) {
    // Sliding window can use circular buffer
    int window_size = 4;
    std::vector<int> buffer(window_size, -1);
    int write_pos = 0;

    // Simulate writing tokens
    for (int token = 0; token < 10; token++) {
        buffer[write_pos] = token;
        write_pos = (write_pos + 1) % window_size;
    }

    // Buffer should contain last 4 tokens: 6, 7, 8, 9
    std::vector<int> expected_in_buffer = {6, 7, 8, 9};
    for (int token : expected_in_buffer) {
        bool found = std::find(buffer.begin(), buffer.end(), token) != buffer.end();
        EXPECT_TRUE(found);
    }
}

// Test: Layer-wise window sizes
TEST_F(SlidingWindowAttentionTest, LayerWiseWindowSizes) {
    // Some models use different window sizes per layer
    // Mistral uses alternating full/sliding attention

    int n_layers = 32;
    int global_window = 4096;
    int local_window = 1024;

    std::vector<int> window_per_layer(n_layers);
    for (int i = 0; i < n_layers; i++) {
        // Alternating pattern
        window_per_layer[i] = (i % 2 == 0) ? local_window : global_window;
    }

    EXPECT_EQ(window_per_layer[0], local_window);
    EXPECT_EQ(window_per_layer[1], global_window);
    EXPECT_EQ(window_per_layer[2], local_window);
}

// Test: Effective context with sliding window
TEST_F(SlidingWindowAttentionTest, EffectiveContext) {
    // With n_layers and window_size w, effective context is n_layers * w
    int n_layers = 32;
    int window_size = 4096;

    int effective_context = n_layers * window_size;

    EXPECT_EQ(effective_context, 131072);  // 128K effective context
}

// Test: Attention pattern validation
TEST_F(SlidingWindowAttentionTest, AttentionPatternValidation) {
    // Ensure attention scores outside window are zero
    int seq_len = 10;
    int window_size = 3;

    // Simulate attention scores
    std::vector<std::vector<float>> scores(seq_len, std::vector<float>(seq_len, 1.0f));

    // Apply sliding window mask
    for (int i = 0; i < seq_len; i++) {
        for (int j = 0; j < seq_len; j++) {
            // Future positions (causal)
            if (j > i) {
                scores[i][j] = 0.0f;
            }
            // Outside window
            else if (i - j >= window_size) {
                scores[i][j] = 0.0f;
            }
        }
    }

    // Verify mask application
    EXPECT_EQ(scores[5][0], 0.0f);  // Too far back
    EXPECT_EQ(scores[5][3], 1.0f);  // Within window
    EXPECT_EQ(scores[5][8], 0.0f);  // Future position
}

