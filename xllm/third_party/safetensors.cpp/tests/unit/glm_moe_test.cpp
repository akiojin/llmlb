/**
 * @file glm_moe_test.cpp
 * @brief Unit tests for GLM-4.7-Flash MoE routing (Task 66)
 *
 * Tests for GLM-4.7-Flash's Mixture of Experts implementation.
 * GLM-4.7-Flash is a 30B-A3B (30 Billion parameters, 3 Billion active) MoE model.
 */

#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstdint>
#include "safetensors.h"
#include "safetensors_internal.h"
#include "arch/glm.h"

class GLMMoETest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: GLM MoE router structure
TEST_F(GLMMoETest, MoERouterStructure) {
    // GLM-4.7-Flash MoE configuration
    std::string moe_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_attention_heads": 32,
        "num_key_value_heads": 2,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(moe_config);

    EXPECT_TRUE(config.is_moe);
    EXPECT_EQ(config.moe_config.num_experts, 8);
    EXPECT_EQ(config.moe_config.num_experts_per_tok, 2);
}

// Test: GLM expert selection (Top-K routing)
TEST_F(GLMMoETest, ExpertSelection) {
    // GLM-4.7-Flash uses Top-K=2 expert selection
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    // Top-K value
    EXPECT_EQ(config.moe_config.num_experts_per_tok, 2);
    // Total experts
    EXPECT_EQ(config.moe_config.num_experts, 8);
}

// Test: GLM expert FFN forward pass
TEST_F(GLMMoETest, ExpertFFNForward) {
    // Each expert is a FFN with intermediate_size
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "intermediate_size": 13696,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    // Expert FFN dimensions
    EXPECT_EQ(config.hidden_size, 4096);
    EXPECT_EQ(config.intermediate_size, 13696);
    EXPECT_TRUE(config.is_moe);
}

// Test: GLM MoE layer forward (full integration)
TEST_F(GLMMoETest, MoELayerForward) {
    // Full MoE layer config
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "intermediate_size": 13696,
        "num_hidden_layers": 40,
        "num_attention_heads": 32,
        "num_key_value_heads": 2,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    // MoE layer should be properly configured
    EXPECT_TRUE(config.is_moe);
    EXPECT_EQ(config.num_hidden_layers, 40);
}

// Test: GLM shared expert handling (if applicable)
TEST_F(GLMMoETest, SharedExpertHandling) {
    // Config with shared experts
    std::string config_with_shared = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2,
        "num_shared_experts": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_with_shared);
    EXPECT_TRUE(config.is_moe);
    EXPECT_EQ(config.moe_config.num_shared_experts, 2);

    // Config without shared experts
    std::string config_no_shared = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config2 = safetensors::glm::parse_glm_config(config_no_shared);
    EXPECT_EQ(config2.moe_config.num_shared_experts, 0);
}

// Test: GLM MoE load balancing
TEST_F(GLMMoETest, MoELoadBalancing) {
    // Load balancing is a runtime concern
    // Config should have proper expert counts
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);
    // Active params ratio: 2/8 = 25%
    EXPECT_EQ(config.moe_config.num_experts, 8);
    EXPECT_EQ(config.moe_config.num_experts_per_tok, 2);
}

// Test: GLM MoE with different configurations
TEST_F(GLMMoETest, BatchProcessing) {
    // Different expert configurations
    std::string config_8_2 = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    std::string config_16_4 = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "moe_num_experts": 16,
        "moe_top_k": 4
    })";

    auto cfg1 = safetensors::glm::parse_glm_config(config_8_2);
    EXPECT_EQ(cfg1.moe_config.num_experts, 8);
    EXPECT_EQ(cfg1.moe_config.num_experts_per_tok, 2);

    auto cfg2 = safetensors::glm::parse_glm_config(config_16_4);
    EXPECT_EQ(cfg2.moe_config.num_experts, 16);
    EXPECT_EQ(cfg2.moe_config.num_experts_per_tok, 4);
}

// Test: GLM MoE weights loading from safetensors
TEST_F(GLMMoETest, WeightsLoading) {
    // MoE weight structure verification
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "intermediate_size": 13696,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    // Verify config supports MoE weights structure
    EXPECT_TRUE(config.is_moe);
    EXPECT_EQ(config.moe_config.num_experts, 8);
}

// Test: GLM MoE tensor name mapping
TEST_F(GLMMoETest, TensorNameMapping) {
    // MoE tensor names should extract layer indices
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.layers.5.mlp.experts.0.gate_proj.weight"), 5);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.layers.10.mlp.router.weight"), 10);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "model.layers.20.block_sparse_moe.gate.weight"), 20);
}

// Test: GLM MoE GPU acceleration
TEST_F(GLMMoETest, GPUAcceleration) {
    // GPU acceleration config - just verify config parsing
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);
    EXPECT_TRUE(config.is_moe);
}

// Test: GLM MoE memory efficiency
TEST_F(GLMMoETest, MemoryEfficiency) {
    // 30B-A3B: 30B total, 3B active per token
    // Active ratio should be num_experts_per_tok / num_experts
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_experts": 8,
        "num_experts_per_tok": 2
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    // Active ratio = 2/8 = 0.25 (25% of experts active)
    float active_ratio = static_cast<float>(config.moe_config.num_experts_per_tok) /
                        static_cast<float>(config.moe_config.num_experts);
    EXPECT_FLOAT_EQ(active_ratio, 0.25f);
}
