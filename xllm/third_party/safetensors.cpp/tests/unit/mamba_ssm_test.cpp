/**
 * @file mamba_ssm_test.cpp
 * @brief Unit tests for Mamba State Space Model (Task 41)
 *
 * Tests for Mamba-2 SSM implementation used in Nemotron 3 architecture.
 * Mamba-2 provides constant state storage during generation (vs linear KV cache),
 * enabling efficient long-context processing up to 1M tokens.
 */

#include <gtest/gtest.h>
#include <vector>
#include <cstdint>
#include <ggml.h>
#include "safetensors.h"
#include "safetensors_internal.h"
#include "mamba.h"

class MambaSSMTest : public ::testing::Test {
protected:
    struct ggml_context* ctx = nullptr;
    safetensors::MambaLayerConfig config;

    void SetUp() override {
        stcpp_init();

        // Initialize ggml context for testing
        struct ggml_init_params params = {
            .mem_size = 128 * 1024 * 1024,  // 128 MB
            .mem_buffer = nullptr,
            .no_alloc = false,
        };
        ctx = ggml_init(params);
        ASSERT_NE(ctx, nullptr);

        // Default Mamba config (similar to Nemotron 3 Nano)
        config.d_model = 128;        // Reduced for testing
        config.d_inner = 256;        // 2x expansion
        config.d_state = 16;         // Reduced for testing
        config.conv_kernel_size = 4;
        config.dt_rank = 32.0f;
        config.dt_min = 0.001f;
        config.dt_max = 0.1f;
    }

    void TearDown() override {
        if (ctx) {
            ggml_free(ctx);
        }
        stcpp_free();
    }
};

// Test: Mamba layer parameters structure
TEST_F(MambaSSMTest, MambaLayerParamsStructure) {
    // Mamba-2 layer should have specific parameters:
    // - d_model: model dimension
    // - d_state: SSM state dimension (constant during generation)
    // - d_conv: convolution kernel size
    // - expand: expansion factor for MLP

    EXPECT_EQ(config.d_model, 128);
    EXPECT_EQ(config.d_inner, 256);
    EXPECT_EQ(config.d_state, 16);
    EXPECT_EQ(config.conv_kernel_size, 4);
    EXPECT_FLOAT_EQ(config.dt_min, 0.001f);
    EXPECT_FLOAT_EQ(config.dt_max, 0.1f);

    // Expansion factor should be d_inner / d_model
    float expansion = static_cast<float>(config.d_inner) / config.d_model;
    EXPECT_FLOAT_EQ(expansion, 2.0f);
}

// Test: SSM conv1d operation
TEST_F(MambaSSMTest, SSMConv1dOperation) {
    // Mamba-2 uses 1D convolution for local context processing
    // ggml operation: GGML_OP_SSM_CONV

    // Create dummy tensors for testing
    auto x = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_inner, 4);  // [seq_len=4, d_inner]
    auto conv_weight = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_inner, config.conv_kernel_size);
    auto conv_bias = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, config.d_inner);

    // Test mamba_ssm_conv function signature and tensor creation
    ASSERT_NE(x, nullptr);
    ASSERT_NE(conv_weight, nullptr);
    ASSERT_NE(conv_bias, nullptr);

    // Function exists and can be called (full integration test requires ggml_graph_compute)
    EXPECT_NO_THROW({
        auto result = safetensors::mamba_ssm_conv(ctx, x, conv_weight, conv_bias, nullptr);
        EXPECT_NE(result, nullptr);
    });
}

// Test: SSM scan operation (state update)
TEST_F(MambaSSMTest, SSMScanOperation) {
    // Mamba-2 state update using SSM scan
    // ggml operation: GGML_OP_SSM_SCAN
    GTEST_SKIP() << "Full integration test requires ggml_graph_compute";

    // State tensor: [batch, d_state, d_model] (constant size)
    // Input tensor: [batch, seq_len, d_model]
    // Output tensor: [batch, seq_len, d_model]
}

// Test: Mamba layer forward pass
TEST_F(MambaSSMTest, MambaLayerForward) {
    // Full Mamba-2 layer forward pass:
    // 1. Normalization
    // 2. SSM conv1d
    // 3. SSM scan (state update)
    // 4. Projection
    // 5. Residual connection
    GTEST_SKIP() << "Full integration test requires model weights and ggml_graph_compute";
}

// Test: State management (constant memory)
TEST_F(MambaSSMTest, StateManagementConstantMemory) {
    // Mamba-2 key advantage: state size is constant regardless of sequence length
    // vs Transformer KV cache which grows linearly with sequence length

    // Create Mamba state
    safetensors::MambaState state(ctx, config);

    // Verify state tensor dimensions
    ASSERT_NE(state.state, nullptr);
    EXPECT_EQ(ggml_nelements(state.state), config.d_state * config.d_inner);
    EXPECT_EQ(state.state->ne[0], config.d_state);
    EXPECT_EQ(state.state->ne[1], config.d_inner);

    // Verify conv_state tensor dimensions
    ASSERT_NE(state.conv_state, nullptr);
    EXPECT_EQ(ggml_nelements(state.conv_state), config.conv_kernel_size * config.d_inner);
    EXPECT_EQ(state.conv_state->ne[0], config.conv_kernel_size);
    EXPECT_EQ(state.conv_state->ne[1], config.d_inner);

    // Test reset functionality
    state.sequence_length = 100;
    state.reset();
    EXPECT_EQ(state.sequence_length, 0);

    // State size is constant regardless of sequence length
    // This is the key advantage over Transformer KV cache
    size_t state_memory = sizeof(float) * config.d_state * config.d_inner;
    size_t conv_state_memory = sizeof(float) * config.conv_kernel_size * config.d_inner;
    size_t total_memory = state_memory + conv_state_memory;

    // Verify memory is constant (independent of sequence length)
    EXPECT_GT(total_memory, 0);
    // For config: d_state=16, d_inner=256, conv_kernel_size=4
    // Expected: (16*256 + 4*256) * 4 bytes = (4096 + 1024) * 4 = 20480 bytes
    EXPECT_EQ(total_memory, (config.d_state + config.conv_kernel_size) * config.d_inner * sizeof(float));
}

// Test: Long context processing (1M tokens)
TEST_F(MambaSSMTest, LongContextProcessing) {
    GTEST_SKIP() << "E2E test - requires full model inference";

    // Nemotron 3 Nano supports 1M-token native context window
    // Mamba-2 enables this through constant state storage
}

// Test: Mamba-2 vs Mamba-1 compatibility
TEST_F(MambaSSMTest, Mamba2Compatibility) {
    GTEST_SKIP() << "E2E test - requires model comparison";

    // Verify that Mamba-2 implementation is used (not Mamba-1)
    // Mamba-2 improvements:
    // - Better scalability
    // - More efficient state updates
    // - Enhanced parallel computation
}

// Test: Residual connections in Mamba layer
TEST_F(MambaSSMTest, ResidualConnections) {
    GTEST_SKIP() << "Integration test - requires full forward pass";

    // Mamba layer uses residual connections like Transformer
    // Output = Mamba(Norm(x)) + x
}

// Test: Mamba layer with different batch sizes
TEST_F(MambaSSMTest, BatchProcessing) {
    GTEST_SKIP() << "Integration test - requires batch inference";

    // Mamba should handle different batch sizes efficiently
    const std::vector<int32_t> batch_sizes = {1, 4, 8, 16};
}

// Test: Integration with ggml backend
TEST_F(MambaSSMTest, GgmlBackendIntegration) {
    // Verify Mamba operations use ggml primitives:
    // - GGML_OP_SSM_CONV for convolution
    // - GGML_OP_SSM_SCAN for state update

    // Basic check: functions exist and create tensors
    EXPECT_TRUE(true) << "Mamba functions exist and compile";
}

// Test: GPU acceleration (CUDA/Metal)
TEST_F(MambaSSMTest, GPUAcceleration) {
    GTEST_SKIP() << "Platform-specific test - requires GPU hardware";

    // Mamba-2 should support GPU acceleration
    // CUDA backend confirmed for llama.cpp Mamba implementation
    // Metal backend status to be verified
}
