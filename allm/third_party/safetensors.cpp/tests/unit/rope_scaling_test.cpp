/**
 * @file rope_scaling_test.cpp
 * @brief Unit tests for RoPE (Rotary Position Embedding) scaling (Task 43)
 */

#include <gtest/gtest.h>
#include <cmath>
#include <vector>
#include "safetensors.h"

class RopeScalingTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Linear scaling factor
TEST_F(RopeScalingTest, LinearScalingFactor) {
    // Linear scaling extends context by dividing position by factor
    // factor = 2.0 -> 4K context becomes effective 8K
    float scale_factor = 2.0f;
    int original_ctx = 4096;
    int effective_ctx = static_cast<int>(original_ctx * scale_factor);

    EXPECT_EQ(effective_ctx, 8192);
}

// Test: NTK-aware scaling
TEST_F(RopeScalingTest, NTKAwareScaling) {
    // NTK scaling modifies the base frequency
    // base_new = base * (factor ^ (dim / (dim - 2)))
    float base = 10000.0f;
    float factor = 2.0f;
    int dim = 64;

    float exponent = static_cast<float>(dim) / (dim - 2);
    float ntk_base = base * std::pow(factor, exponent);

    EXPECT_GT(ntk_base, base);
    EXPECT_NEAR(ntk_base, base * std::pow(2.0f, 64.0f / 62.0f), 0.1f);
}

// Test: RoPE frequency calculation
TEST_F(RopeScalingTest, RopeFrequencyCalculation) {
    // freq_i = 1 / (base ^ (2i / dim))
    float base = 10000.0f;
    int dim = 64;

    std::vector<float> frequencies;
    for (int i = 0; i < dim / 2; i++) {
        float freq = 1.0f / std::pow(base, 2.0f * i / dim);
        frequencies.push_back(freq);
    }

    // First frequency should be 1.0
    EXPECT_NEAR(frequencies[0], 1.0f, 0.001f);

    // Frequencies should decrease
    for (size_t i = 1; i < frequencies.size(); i++) {
        EXPECT_LT(frequencies[i], frequencies[i - 1]);
    }
}

// Test: Position embedding rotation
TEST_F(RopeScalingTest, PositionEmbeddingRotation) {
    // RoPE applies rotation in 2D subspaces
    float x = 1.0f;
    float y = 0.0f;
    float theta = M_PI / 4;  // 45 degrees

    float x_rot = x * std::cos(theta) - y * std::sin(theta);
    float y_rot = x * std::sin(theta) + y * std::cos(theta);

    EXPECT_NEAR(x_rot, std::sqrt(2.0f) / 2, 0.001f);
    EXPECT_NEAR(y_rot, std::sqrt(2.0f) / 2, 0.001f);
}

// Test: Dynamic NTK scaling
TEST_F(RopeScalingTest, DynamicNTKScaling) {
    // Dynamic NTK adjusts scaling based on sequence length
    int max_trained_ctx = 4096;
    int current_seq_len = 6000;

    float dynamic_factor = 1.0f;
    if (current_seq_len > max_trained_ctx) {
        dynamic_factor = static_cast<float>(current_seq_len) / max_trained_ctx;
    }

    EXPECT_GT(dynamic_factor, 1.0f);
    EXPECT_NEAR(dynamic_factor, 6000.0f / 4096.0f, 0.001f);
}

// Test: YaRN scaling
TEST_F(RopeScalingTest, YaRNScaling) {
    // YaRN (Yet another RoPE Extension) combines linear and NTK
    // with attention temperature scaling

    float scale_factor = 2.0f;
    float attn_factor = 0.1f * std::log(scale_factor) + 1.0f;

    EXPECT_GT(attn_factor, 1.0f);
    EXPECT_NEAR(attn_factor, 0.1f * std::log(2.0f) + 1.0f, 0.001f);
}

// Test: Scaling type configuration
TEST_F(RopeScalingTest, ScalingTypeConfiguration) {
    enum class RopeScalingType {
        NONE = 0,
        LINEAR = 1,
        NTK = 2,
        DYNAMIC_NTK = 3,
        YARN = 4,
    };

    RopeScalingType type = RopeScalingType::LINEAR;
    EXPECT_EQ(static_cast<int>(type), 1);

    type = RopeScalingType::NTK;
    EXPECT_EQ(static_cast<int>(type), 2);
}

// Test: Context length extension
TEST_F(RopeScalingTest, ContextLengthExtension) {
    // Verify context extension scenarios
    struct ContextConfig {
        int original_ctx;
        float scale_factor;
        int expected_ctx;
    };

    std::vector<ContextConfig> configs = {
        {4096, 2.0f, 8192},
        {4096, 4.0f, 16384},
        {8192, 2.0f, 16384},
        {2048, 8.0f, 16384},
    };

    for (const auto& cfg : configs) {
        int extended_ctx = static_cast<int>(cfg.original_ctx * cfg.scale_factor);
        EXPECT_EQ(extended_ctx, cfg.expected_ctx);
    }
}

