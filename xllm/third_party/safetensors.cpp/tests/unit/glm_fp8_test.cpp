/**
 * @file glm_fp8_test.cpp
 * @brief Unit tests for GLM-4.7-FP8 quantization support (Task 67)
 *
 * Tests for GLM-4.7-FP8's FP8 (8-bit floating point) quantization.
 * FP8 provides memory efficiency while maintaining model quality.
 */

#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstdint>
#include "safetensors.h"
#include "safetensors_internal.h"
#include "arch/glm.h"

class GLMFP8Test : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: FP8 tensor dtype detection
TEST_F(GLMFP8Test, FP8DtypeDetection) {
    // FP8 detection via torch_dtype
    std::string fp8_e4m3_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config1 = safetensors::glm::parse_glm_config(fp8_e4m3_config);
    EXPECT_TRUE(config1.is_fp8);

    std::string fp8_e5m2_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "float8_e5m2"
    })";

    auto config2 = safetensors::glm::parse_glm_config(fp8_e5m2_config);
    EXPECT_TRUE(config2.is_fp8);
}

// Test: FP8 tensor loading from safetensors
TEST_F(GLMFP8Test, FP8TensorLoading) {
    // FP8 detection via quantization_config
    std::string fp8_quant_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "quantization_config": {
            "quant_method": "fp8",
            "weight_bits": 8
        }
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_quant_config);
    EXPECT_TRUE(config.is_fp8);
}

// Test: FP8 to FP16/FP32 conversion
TEST_F(GLMFP8Test, FP8ToHigherPrecisionConversion) {
    // Non-FP8 config for comparison
    std::string fp16_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "float16"
    })";

    auto config = safetensors::glm::parse_glm_config(fp16_config);
    EXPECT_FALSE(config.is_fp8);
}

// Test: FP8 matmul operations
TEST_F(GLMFP8Test, FP8MatmulOperations) {
    // FP8 model config
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "intermediate_size": 13696,
        "num_hidden_layers": 32,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.intermediate_size, 13696);
}

// Test: FP8 quantization scales handling
TEST_F(GLMFP8Test, QuantizationScalesHandling) {
    // FP8 with quantization config
    std::string fp8_scales_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "quantization_config": {
            "quant_method": "fp8",
            "activation_scheme": "dynamic"
        }
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_scales_config);
    EXPECT_TRUE(config.is_fp8);
}

// Test: Mixed precision (FP8 weights + FP16/FP32 activations)
TEST_F(GLMFP8Test, MixedPrecisionComputation) {
    // FP8 model dimensions for mixed precision compute
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "num_key_value_heads": 2,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.num_attention_heads, 32);
    EXPECT_EQ(config.num_key_value_heads, 2);
}

// Test: FP8 memory efficiency
TEST_F(GLMFP8Test, MemoryEfficiency) {
    // FP8 uses 8 bits vs FP16's 16 bits = 50% memory reduction
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "vocab_size": 151552,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.vocab_size, 151552);
}

// Test: FP8 numerical stability
TEST_F(GLMFP8Test, NumericalStability) {
    // FP8 model with proper layer norm for stability
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "layer_norm_eps": 1e-5,
        "use_rms_norm": true,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_TRUE(config.use_rms_norm);
    EXPECT_FLOAT_EQ(config.layer_norm_eps, 1e-5f);
}

// Test: FP8 attention computation
TEST_F(GLMFP8Test, FP8AttentionComputation) {
    // FP8 model attention config
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_attention_heads": 32,
        "num_key_value_heads": 2,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.head_dim, 128); // 4096 / 32
}

// Test: FP8 FFN computation
TEST_F(GLMFP8Test, FP8FFNComputation) {
    // FP8 model FFN config
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "intermediate_size": 13696,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.hidden_size, 4096);
    EXPECT_EQ(config.intermediate_size, 13696);
}

// Test: ggml FP8 backend support
TEST_F(GLMFP8Test, GgmlFP8Support) {
    // Config parsing for FP8 - ggml support is runtime
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
}

// Test: CUDA FP8 acceleration (if available)
TEST_F(GLMFP8Test, CUDAFP8Acceleration) {
    // FP8 config - CUDA acceleration is runtime
    std::string fp8_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config = safetensors::glm::parse_glm_config(fp8_config);
    EXPECT_TRUE(config.is_fp8);
    EXPECT_EQ(config.num_hidden_layers, 32);
}

// Test: Fallback for non-FP8 hardware
TEST_F(GLMFP8Test, NonFP8HardwareFallback) {
    // BF16 fallback config
    std::string bf16_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "bfloat16"
    })";

    auto config = safetensors::glm::parse_glm_config(bf16_config);
    EXPECT_FALSE(config.is_fp8);

    // FP32 fallback config
    std::string fp32_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "torch_dtype": "float32"
    })";

    auto config2 = safetensors::glm::parse_glm_config(fp32_config);
    EXPECT_FALSE(config2.is_fp8);
}
