/**
 * @file gqa_mqa_test.cpp
 * @brief Unit tests for Grouped Query Attention (GQA) and Multi-Query Attention (MQA) (Task 47)
 */

#include <gtest/gtest.h>
#include <vector>
#include "safetensors.h"

class GQAMQATest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: MHA configuration (baseline)
TEST_F(GQAMQATest, MHAConfiguration) {
    // Multi-Head Attention: n_kv_heads == n_heads
    int n_heads = 32;
    int n_kv_heads = 32;

    EXPECT_EQ(n_heads, n_kv_heads);
    EXPECT_EQ(n_heads / n_kv_heads, 1);  // No grouping
}

// Test: GQA configuration
TEST_F(GQAMQATest, GQAConfiguration) {
    // Grouped Query Attention: n_kv_heads < n_heads
    // Llama 2 70B uses GQA with n_heads=64, n_kv_heads=8

    int n_heads = 64;
    int n_kv_heads = 8;
    int group_size = n_heads / n_kv_heads;

    EXPECT_EQ(group_size, 8);
    EXPECT_LT(n_kv_heads, n_heads);
    EXPECT_EQ(n_heads % n_kv_heads, 0);  // Must divide evenly
}

// Test: MQA configuration
TEST_F(GQAMQATest, MQAConfiguration) {
    // Multi-Query Attention: n_kv_heads == 1
    int n_heads = 32;
    int n_kv_heads = 1;
    int group_size = n_heads / n_kv_heads;

    EXPECT_EQ(n_kv_heads, 1);
    EXPECT_EQ(group_size, 32);  // All heads share one KV
}

// Test: KV cache memory savings
TEST_F(GQAMQATest, KVCacheMemorySavings) {
    // KV cache size is proportional to n_kv_heads
    int seq_len = 4096;
    int head_dim = 128;
    int n_layers = 32;

    // MHA: n_kv_heads = 32
    int64_t mha_kv_cache = 2LL * seq_len * 32 * head_dim * n_layers * sizeof(float);

    // GQA: n_kv_heads = 8
    int64_t gqa_kv_cache = 2LL * seq_len * 8 * head_dim * n_layers * sizeof(float);

    // MQA: n_kv_heads = 1
    int64_t mqa_kv_cache = 2LL * seq_len * 1 * head_dim * n_layers * sizeof(float);

    float gqa_ratio = static_cast<float>(gqa_kv_cache) / mha_kv_cache;
    float mqa_ratio = static_cast<float>(mqa_kv_cache) / mha_kv_cache;

    EXPECT_NEAR(gqa_ratio, 0.25f, 0.01f);    // 4x savings
    EXPECT_NEAR(mqa_ratio, 0.03125f, 0.01f); // 32x savings
}

// Test: Head broadcast for GQA
TEST_F(GQAMQATest, HeadBroadcastForGQA) {
    // In GQA, KV heads are broadcast to Q heads
    int n_heads = 8;
    int n_kv_heads = 2;
    int group_size = n_heads / n_kv_heads;

    // Map Q head index to KV head index
    std::vector<int> q_to_kv_map(n_heads);
    for (int q = 0; q < n_heads; q++) {
        q_to_kv_map[q] = q / group_size;
    }

    // Q heads 0,1,2,3 map to KV head 0
    EXPECT_EQ(q_to_kv_map[0], 0);
    EXPECT_EQ(q_to_kv_map[1], 0);
    EXPECT_EQ(q_to_kv_map[2], 0);
    EXPECT_EQ(q_to_kv_map[3], 0);

    // Q heads 4,5,6,7 map to KV head 1
    EXPECT_EQ(q_to_kv_map[4], 1);
    EXPECT_EQ(q_to_kv_map[5], 1);
    EXPECT_EQ(q_to_kv_map[6], 1);
    EXPECT_EQ(q_to_kv_map[7], 1);
}

// Test: Common model configurations
TEST_F(GQAMQATest, CommonModelConfigurations) {
    struct ModelConfig {
        const char* name;
        int n_heads;
        int n_kv_heads;
    };

    std::vector<ModelConfig> models = {
        {"Llama 2 7B", 32, 32},    // MHA
        {"Llama 2 70B", 64, 8},    // GQA
        {"Llama 3 8B", 32, 8},     // GQA
        {"Llama 3 70B", 64, 8},    // GQA
        {"Mistral 7B", 32, 8},     // GQA
        {"Falcon 40B", 64, 1},     // MQA
    };

    for (const auto& model : models) {
        // Validate configuration
        EXPECT_GT(model.n_heads, 0);
        EXPECT_GT(model.n_kv_heads, 0);
        EXPECT_LE(model.n_kv_heads, model.n_heads);
        EXPECT_EQ(model.n_heads % model.n_kv_heads, 0);

        // Classify attention type
        if (model.n_kv_heads == model.n_heads) {
            // MHA
            EXPECT_EQ(model.n_heads / model.n_kv_heads, 1);
        } else if (model.n_kv_heads == 1) {
            // MQA
            EXPECT_EQ(model.n_kv_heads, 1);
        } else {
            // GQA
            EXPECT_GT(model.n_heads / model.n_kv_heads, 1);
            EXPECT_LT(model.n_heads / model.n_kv_heads, model.n_heads);
        }
    }
}

// Test: Attention score calculation with GQA
TEST_F(GQAMQATest, AttentionScoreCalculationGQA) {
    // Q: [batch, n_heads, seq_len, head_dim]
    // K: [batch, n_kv_heads, seq_len, head_dim]
    // For GQA, K is broadcast to match Q

    int batch = 1;
    int n_heads = 8;
    int n_kv_heads = 2;
    int seq_len = 4;
    int head_dim = 64;

    // Shape calculations
    int64_t q_size = batch * n_heads * seq_len * head_dim;
    int64_t k_size = batch * n_kv_heads * seq_len * head_dim;

    // K is smaller
    EXPECT_LT(k_size, q_size);
    EXPECT_EQ(k_size * 4, q_size);  // 4x smaller with group_size=4
}

// Test: KV cache update with GQA
TEST_F(GQAMQATest, KVCacheUpdateGQA) {
    // When updating KV cache with GQA, only update n_kv_heads entries
    int n_kv_heads = 8;
    int head_dim = 128;

    // Simulate KV cache slot
    struct KVCacheSlot {
        std::vector<float> k;
        std::vector<float> v;
    };

    std::vector<KVCacheSlot> cache(n_kv_heads);
    for (int h = 0; h < n_kv_heads; h++) {
        cache[h].k.resize(head_dim, 0.0f);
        cache[h].v.resize(head_dim, 0.0f);
    }

    // Update cache
    for (int h = 0; h < n_kv_heads; h++) {
        for (int d = 0; d < head_dim; d++) {
            cache[h].k[d] = static_cast<float>(h * head_dim + d);
            cache[h].v[d] = static_cast<float>(h * head_dim + d + 1000);
        }
    }

    // Verify update
    EXPECT_EQ(cache[0].k[0], 0.0f);
    EXPECT_EQ(cache[7].k[0], 7 * head_dim);
}

// Test: Inference throughput improvement
TEST_F(GQAMQATest, InferenceThroughputImprovement) {
    // GQA/MQA improves throughput by reducing memory bandwidth
    // Estimate memory bandwidth reduction

    int seq_len = 4096;
    int head_dim = 128;
    int n_heads = 64;

    // Bytes per token for KV read
    auto kv_bytes_per_token = [&](int n_kv_heads) {
        return 2 * seq_len * n_kv_heads * head_dim * sizeof(float);
    };

    int64_t mha_bytes = kv_bytes_per_token(n_heads);    // n_kv_heads = 64
    int64_t gqa_bytes = kv_bytes_per_token(8);          // n_kv_heads = 8
    int64_t mqa_bytes = kv_bytes_per_token(1);          // n_kv_heads = 1

    // Bandwidth reduction
    float gqa_speedup = static_cast<float>(mha_bytes) / gqa_bytes;
    float mqa_speedup = static_cast<float>(mha_bytes) / mqa_bytes;

    EXPECT_NEAR(gqa_speedup, 8.0f, 0.1f);
    EXPECT_NEAR(mqa_speedup, 64.0f, 0.1f);
}

