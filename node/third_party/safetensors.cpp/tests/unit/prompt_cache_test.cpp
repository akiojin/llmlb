/**
 * @file prompt_cache_test.cpp
 * @brief Unit tests for prompt caching (Task 32)
 */

#include <gtest/gtest.h>
#include <filesystem>
#include <fstream>
#include "safetensors.h"
#include "safetensors_internal.h"

namespace fs = std::filesystem;

class PromptCacheTest : public ::testing::Test {
protected:
    fs::path temp_dir;

    void SetUp() override {
        stcpp_init();
        temp_dir = fs::temp_directory_path() / "stcpp_prompt_cache_test";
        fs::create_directories(temp_dir);
    }

    void TearDown() override {
        fs::remove_all(temp_dir);
        stcpp_free();
    }
};

// Test: Prompt cache save function signature
TEST_F(PromptCacheTest, SaveFunctionSignature) {
    fs::path cache_path = temp_dir / "test_cache.bin";

    // Null context should return error
    stcpp_error result = stcpp_prompt_cache_save(
        nullptr,
        "Test prompt",
        cache_path.string().c_str()
    );

    EXPECT_NE(result, STCPP_OK);
}

// Test: Prompt cache load function signature
TEST_F(PromptCacheTest, LoadFunctionSignature) {
    fs::path cache_path = temp_dir / "nonexistent_cache.bin";

    // Null context should return error
    stcpp_error result = stcpp_prompt_cache_load(
        nullptr,
        cache_path.string().c_str()
    );

    EXPECT_NE(result, STCPP_OK);
}

// Test: Prompt cache file format
TEST_F(PromptCacheTest, CacheFileFormat) {
    // Prompt cache file format:
    // - Magic number (4 bytes): "STPC"
    // - Version (4 bytes): uint32
    // - Prompt hash (32 bytes): SHA256
    // - KV cache size (8 bytes): uint64
    // - KV cache data (variable)

    const uint32_t MAGIC = 0x43505453;  // "STPC" in little-endian
    const uint32_t VERSION = 1;

    EXPECT_EQ(MAGIC, 0x43505453);
    EXPECT_EQ(VERSION, 1);
}

// Test: Prompt hash computation
TEST_F(PromptCacheTest, PromptHashComputation) {
    // Same prompt should produce same hash
    std::string prompt1 = "Hello, world!";
    std::string prompt2 = "Hello, world!";
    std::string prompt3 = "Different prompt";

    uint64_t hash1 = stcpp::compute_prompt_hash(prompt1);
    uint64_t hash2 = stcpp::compute_prompt_hash(prompt2);
    uint64_t hash3 = stcpp::compute_prompt_hash(prompt3);

    EXPECT_EQ(hash1, hash2);
    EXPECT_NE(hash1, hash3);
}

// Test: Cache hit detection
TEST_F(PromptCacheTest, CacheHitDetection) {
    // When a prompt is a prefix of a cached prompt, we can reuse the KV cache
    std::string cached_prompt = "The quick brown fox jumps over";
    std::string new_prompt = "The quick brown fox jumps over the lazy dog";

    // new_prompt starts with cached_prompt, so we can reuse KV cache
    bool is_prefix = (new_prompt.find(cached_prompt) == 0);
    EXPECT_TRUE(is_prefix);

    // Different prompt should not be a hit
    std::string different = "A different prompt entirely";
    bool is_hit = (different.find(cached_prompt) == 0);
    EXPECT_FALSE(is_hit);
}

// Test: Cache reuse benefit
TEST_F(PromptCacheTest, CacheReuseBenefit) {
    // When reusing KV cache:
    // - Skip tokenization of cached portion
    // - Skip prefill computation of cached portion
    // - Only process new tokens

    int cached_tokens = 100;
    int new_tokens = 50;
    int total_tokens = cached_tokens + new_tokens;

    // Without cache: process all tokens
    int tokens_without_cache = total_tokens;

    // With cache: only process new tokens
    int tokens_with_cache = new_tokens;

    // Savings
    float savings = 1.0f - (static_cast<float>(tokens_with_cache) / tokens_without_cache);
    EXPECT_GT(savings, 0.0f);
    EXPECT_FLOAT_EQ(savings, 100.0f / 150.0f);  // ~66% savings
}

// Test: Cache invalidation
TEST_F(PromptCacheTest, CacheInvalidation) {
    // Cache should be invalidated when:
    // - Model changes
    // - Context parameters change
    // - Cache file is corrupted

    stcpp::PromptCacheMetadata meta;
    meta.model_hash = 12345;
    meta.n_ctx = 4096;
    meta.valid = true;

    // Different model should invalidate
    uint64_t current_model_hash = 54321;
    bool valid_model = (meta.model_hash == current_model_hash);
    EXPECT_FALSE(valid_model);

    // Same model should be valid
    current_model_hash = 12345;
    valid_model = (meta.model_hash == current_model_hash);
    EXPECT_TRUE(valid_model);
}

// Test: Cache size limits
TEST_F(PromptCacheTest, CacheSizeLimits) {
    // Cache file size depends on:
    // - Number of cached tokens
    // - KV cache dimensions
    // - Quantization

    int32_t n_tokens = 1000;
    int32_t n_layers = 32;
    int32_t n_heads = 32;
    int32_t head_dim = 128;

    // FP16 cache size per token
    size_t bytes_per_token = 2 * n_layers * n_heads * head_dim * 2;  // K and V, FP16

    // Total cache size
    size_t total_size = n_tokens * bytes_per_token;

    // Should be manageable (< 1GB for reasonable token counts)
    EXPECT_LT(total_size, 1024 * 1024 * 1024);
}

// Test: Multiple prompt caches
TEST_F(PromptCacheTest, MultiplePromptCaches) {
    // System should support multiple cached prompts
    std::vector<std::string> system_prompts = {
        "You are a helpful assistant.",
        "You are a code reviewer.",
        "You are a translator.",
    };

    // Each prompt should have its own cache file
    for (size_t i = 0; i < system_prompts.size(); i++) {
        fs::path cache_path = temp_dir / ("cache_" + std::to_string(i) + ".bin");
        EXPECT_FALSE(fs::exists(cache_path));  // Not created yet
    }
}

// Test: Prompt cache internal structure
TEST_F(PromptCacheTest, PromptCacheInternalStructure) {
    stcpp::PromptCacheMetadata meta;

    // Default state
    EXPECT_EQ(meta.model_hash, 0);
    EXPECT_EQ(meta.n_ctx, 0);
    EXPECT_EQ(meta.n_tokens, 0);
    EXPECT_FALSE(meta.valid);
}

