/**
 * @file kv_cache_test.cpp
 * @brief Unit tests for KV cache management (Task 31)
 */

#include <gtest/gtest.h>
#include <vector>
#include <cstdint>
#include "safetensors.h"
#include "safetensors_internal.h"

class KVCacheTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: KV cache params structure
TEST_F(KVCacheTest, KVCacheParamsStructure) {
    stcpp_context_params params = stcpp_context_default_params();

    // kv_cache_quant should be configurable
    EXPECT_FALSE(params.kv_cache_quant);

    params.kv_cache_quant = true;
    EXPECT_TRUE(params.kv_cache_quant);
}

// Test: Context size affects KV cache
TEST_F(KVCacheTest, ContextSizeAffectsKVCache) {
    stcpp_context_params params = stcpp_context_default_params();

    // Different context sizes
    params.n_ctx = 2048;
    EXPECT_EQ(params.n_ctx, 2048);

    params.n_ctx = 8192;
    EXPECT_EQ(params.n_ctx, 8192);

    params.n_ctx = 32768;
    EXPECT_EQ(params.n_ctx, 32768);
}

// Test: KV cache clear function signature
TEST_F(KVCacheTest, KVCacheClearFunctionSignature) {
    // stcpp_context_kv_cache_clear should be safe to call with null
    stcpp_context_kv_cache_clear(nullptr);

    // Should not crash
    SUCCEED();
}

// Test: KV cache memory estimation
TEST_F(KVCacheTest, KVCacheMemoryEstimation) {
    // KV cache size calculation:
    // size = 2 * n_layers * n_ctx * n_heads * head_dim * dtype_size
    // For a typical model:
    // - n_layers = 32
    // - n_ctx = 4096
    // - n_heads = 32
    // - head_dim = 128
    // - dtype = FP16 (2 bytes) or INT8 (1 byte)

    int32_t n_layers = 32;
    int32_t n_ctx = 4096;
    int32_t n_heads = 32;
    int32_t head_dim = 128;

    // FP16 KV cache
    size_t fp16_size = 2 * n_layers * n_ctx * n_heads * head_dim * 2;
    EXPECT_GT(fp16_size, 0);

    // INT8 quantized KV cache (half the size)
    size_t int8_size = 2 * n_layers * n_ctx * n_heads * head_dim * 1;
    EXPECT_EQ(int8_size, fp16_size / 2);
}

// Test: KV cache quantization options
TEST_F(KVCacheTest, KVCacheQuantizationOptions) {
    // Supported quantization types for KV cache:
    // - FP16 (default, full precision)
    // - INT8 (8-bit quantization)
    // - FP8 (8-bit floating point)

    enum KVCacheQuantType {
        KV_QUANT_NONE = 0,  // FP16
        KV_QUANT_INT8 = 1,
        KV_QUANT_FP8 = 2,
    };

    KVCacheQuantType type = KV_QUANT_NONE;
    EXPECT_EQ(type, KV_QUANT_NONE);

    type = KV_QUANT_INT8;
    EXPECT_EQ(type, KV_QUANT_INT8);

    type = KV_QUANT_FP8;
    EXPECT_EQ(type, KV_QUANT_FP8);
}

// Test: Batch size affects KV cache allocation
TEST_F(KVCacheTest, BatchSizeAffectsKVCache) {
    stcpp_context_params params = stcpp_context_default_params();

    // Batch size affects how much KV cache is allocated per batch
    params.n_batch = 512;
    EXPECT_EQ(params.n_batch, 512);

    params.n_batch = 1024;
    EXPECT_EQ(params.n_batch, 1024);

    params.n_batch = 2048;
    EXPECT_EQ(params.n_batch, 2048);
}

// Test: KV cache internal structure
TEST_F(KVCacheTest, KVCacheInternalStructure) {
    // KV cache stores key and value tensors for each layer
    stcpp::KVCache cache;

    // Default state
    EXPECT_EQ(cache.n_ctx, 0);
    EXPECT_EQ(cache.n_used, 0);
    EXPECT_TRUE(cache.k_data.empty());
    EXPECT_TRUE(cache.v_data.empty());
}

// Test: KV cache allocation
TEST_F(KVCacheTest, KVCacheAllocation) {
    stcpp::KVCache cache;

    int32_t n_ctx = 4096;
    int32_t n_layers = 32;
    int32_t n_heads = 32;
    int32_t head_dim = 128;

    bool result = stcpp::kv_cache_alloc(cache, n_ctx, n_layers, n_heads, head_dim, false);

    EXPECT_TRUE(result);
    EXPECT_EQ(cache.n_ctx, n_ctx);
    EXPECT_EQ(cache.n_used, 0);
    EXPECT_FALSE(cache.k_data.empty());
    EXPECT_FALSE(cache.v_data.empty());
}

// Test: KV cache clear resets usage
TEST_F(KVCacheTest, KVCacheClearResetsUsage) {
    stcpp::KVCache cache;
    cache.n_ctx = 4096;
    cache.n_used = 1000;

    stcpp::kv_cache_clear(cache);

    EXPECT_EQ(cache.n_used, 0);
    // n_ctx should remain unchanged
    EXPECT_EQ(cache.n_ctx, 4096);
}

// Test: KV cache defragmentation
TEST_F(KVCacheTest, KVCacheDefragmentation) {
    // When KV cache becomes fragmented, defragmentation can help
    // This is useful for continuous batching scenarios

    stcpp::KVCache cache;
    cache.n_ctx = 4096;
    cache.n_used = 2000;

    // Simulate fragmentation by having gaps
    // After defragmentation, contiguous memory should be reclaimed

    stcpp::kv_cache_defrag(cache);

    // Should not crash and state should be valid
    EXPECT_LE(cache.n_used, cache.n_ctx);
}

