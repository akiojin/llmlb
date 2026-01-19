#include <gtest/gtest.h>
#include "../../src/gqa.h"
#include "../../src/safetensors.h"
#include <ggml.h>

using namespace safetensors;

class GQATest : public ::testing::Test {
protected:
    struct ggml_context* ctx = nullptr;
    GQALayerConfig config;

    void SetUp() override {
        stcpp_init();

        struct ggml_init_params params = {
            .mem_size = 128 * 1024 * 1024,  // 128 MB
            .mem_buffer = nullptr,
            .no_alloc = false,
        };
        ctx = ggml_init(params);
        ASSERT_NE(ctx, nullptr);

        // Nemotron 3 Nano GQA configuration
        config.d_model = 2560;
        config.n_heads = 32;
        config.n_kv_groups = 2;  // Nemotron 3 uses 2 KV groups
        config.head_dim = config.d_model / config.n_heads;  // 80
        config.max_seq_len = 1048576;  // 1M tokens
    }

    void TearDown() override {
        if (ctx) {
            ggml_free(ctx);
            ctx = nullptr;
        }
    }
};

// Test 1: GQA Configuration Validation
TEST_F(GQATest, ConfigurationValidation) {
    EXPECT_EQ(config.d_model, 2560);
    EXPECT_EQ(config.n_heads, 32);
    EXPECT_EQ(config.n_kv_groups, 2);
    EXPECT_EQ(config.head_dim, 80);  // 2560 / 32
    EXPECT_EQ(config.max_seq_len, 1048576);

    // Verify that n_heads is divisible by n_kv_groups
    EXPECT_EQ(config.n_heads % config.n_kv_groups, 0);
}

// Test 2: KV Cache Allocation
TEST_F(GQATest, KVCacheAllocation) {
    GQAKVCache cache(ctx, config);

    ASSERT_NE(cache.k_cache, nullptr);
    ASSERT_NE(cache.v_cache, nullptr);

    int kv_dim = config.n_kv_groups * config.head_dim;  // 2 * 80 = 160
    EXPECT_EQ(ggml_nelements(cache.k_cache), kv_dim * config.max_seq_len);
    EXPECT_EQ(ggml_nelements(cache.v_cache), kv_dim * config.max_seq_len);

    EXPECT_EQ(cache.current_seq_len, 0);
}

// Test 3: KV Cache Reset
TEST_F(GQATest, KVCacheReset) {
    GQAKVCache cache(ctx, config);

    cache.current_seq_len = 100;
    cache.reset();

    EXPECT_EQ(cache.current_seq_len, 0);
}

// Test 4: GQA Layer Weights Structure
TEST_F(GQATest, LayerWeightsStructure) {
    GQALayerWeights weights{};

    // Allocate weight tensors
    weights.norm_weight = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, config.d_model);
    weights.q_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, config.d_model);

    int kv_dim = config.n_kv_groups * config.head_dim;
    weights.k_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, kv_dim);
    weights.v_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, kv_dim);
    weights.o_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, config.d_model);

    ASSERT_NE(weights.norm_weight, nullptr);
    ASSERT_NE(weights.q_proj_weight, nullptr);
    ASSERT_NE(weights.k_proj_weight, nullptr);
    ASSERT_NE(weights.v_proj_weight, nullptr);
    ASSERT_NE(weights.o_proj_weight, nullptr);

    // Verify dimensions
    EXPECT_EQ(ggml_nelements(weights.norm_weight), config.d_model);
    EXPECT_EQ(ggml_nelements(weights.q_proj_weight), config.d_model * config.d_model);
    EXPECT_EQ(ggml_nelements(weights.k_proj_weight), config.d_model * kv_dim);
}

// Test 5: RoPE Application (stub test)
TEST_F(GQATest, RoPEApplication) {
    GTEST_SKIP() << "RoPE requires GPU execution context";

    int seq_len = 4;
    int batch_size = 1;

    struct ggml_tensor* x = ggml_new_tensor_4d(ctx, GGML_TYPE_F32,
        config.head_dim, config.n_heads, seq_len, batch_size);
    ASSERT_NE(x, nullptr);

    EXPECT_NO_THROW({
        struct ggml_tensor* x_rope = apply_rope_gqa(ctx, x, 0, config.head_dim, 0,
            config.max_seq_len, config.rope_theta, config.rope_freq_scale);
        EXPECT_NE(x_rope, nullptr);
    });
}

// Test 6: GQA Layer Forward Pass (stub test)
TEST_F(GQATest, ForwardPassSmoke) {
    GTEST_SKIP() << "Forward pass requires complete model weights and GPU execution";

    GQALayerWeights weights{};
    GQAKVCache cache(ctx, config);

    // Allocate minimal weights
    weights.q_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, config.d_model);
    weights.k_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model,
        config.n_kv_groups * config.head_dim);
    weights.v_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model,
        config.n_kv_groups * config.head_dim);
    weights.o_proj_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, config.d_model);

    struct ggml_tensor* input = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_model, 1);
    ASSERT_NE(input, nullptr);

    EXPECT_NO_THROW({
        struct ggml_tensor* output = gqa_layer_forward(ctx, input, weights, &cache, config, 0);
        EXPECT_NE(output, nullptr);
    });
}

// Test 7: Grouped KV Dimensions
TEST_F(GQATest, GroupedKVDimensions) {
    // Verify GQA dimension calculations
    int heads_per_group = config.n_heads / config.n_kv_groups;  // 32 / 2 = 16
    EXPECT_EQ(heads_per_group, 16);

    int total_kv_dim = config.n_kv_groups * config.head_dim;  // 2 * 80 = 160
    int total_q_dim = config.n_heads * config.head_dim;  // 32 * 80 = 2560

    EXPECT_EQ(total_kv_dim, 160);
    EXPECT_EQ(total_q_dim, config.d_model);
}
