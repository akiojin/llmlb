/**
 * @file glm_config_test.cpp
 * @brief Unit tests for GLM-4.7 config.json parsing (Task 65)
 *
 * Tests for GLM-4.7 architecture configuration parsing.
 * GLM-4.7 is distributed by Z.ai (zai-org) in safetensors format.
 *
 * Models:
 * - zai-org/GLM-4.7: Full model (717GB)
 * - zai-org/GLM-4.7-Flash: 30B-A3B MoE (lightweight)
 * - zai-org/GLM-4.7-FP8: FP8 quantized version
 */

#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstdint>
#include "safetensors.h"
#include "safetensors_internal.h"
#include "arch/glm.h"

class GLMConfigTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: GLM architecture detection from config.json
TEST_F(GLMConfigTest, ArchitectureDetection) {
    // GLM-4.7 config.json should contain:
    // "architectures": ["ChatGLMForCausalLM"] or similar
    // "model_type": "chatglm" or "glm"

    // Valid GLM config with architectures field (GLM4)
    std::string glm4_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32
    })";
    EXPECT_TRUE(safetensors::glm::is_glm_architecture(glm4_config));

    // Valid GLM config with model_type field
    std::string glm_model_type = R"({
        "model_type": "glm4",
        "hidden_size": 4096
    })";
    EXPECT_TRUE(safetensors::glm::is_glm_architecture(glm_model_type));

    // ChatGLM variant
    std::string chatglm_config = R"({
        "architectures": ["ChatGLMForConditionalGeneration"],
        "hidden_size": 4096
    })";
    EXPECT_TRUE(safetensors::glm::is_glm_architecture(chatglm_config));

    // chatglm model_type
    std::string chatglm_type = R"({
        "model_type": "chatglm",
        "hidden_size": 4096
    })";
    EXPECT_TRUE(safetensors::glm::is_glm_architecture(chatglm_type));

    // Non-GLM config (Llama)
    std::string llama_config = R"({
        "architectures": ["LlamaForCausalLM"],
        "model_type": "llama"
    })";
    EXPECT_FALSE(safetensors::glm::is_glm_architecture(llama_config));

    // Non-GLM config (Mistral)
    std::string mistral_config = R"({
        "architectures": ["MistralForCausalLM"],
        "model_type": "mistral"
    })";
    EXPECT_FALSE(safetensors::glm::is_glm_architecture(mistral_config));

    // Invalid JSON should return false (not crash)
    std::string invalid_json = "not valid json {{{";
    EXPECT_FALSE(safetensors::glm::is_glm_architecture(invalid_json));

    // Empty JSON
    std::string empty_config = "{}";
    EXPECT_FALSE(safetensors::glm::is_glm_architecture(empty_config));
}

// Test: GLM-4.7-Flash config parsing (MoE configuration)
TEST_F(GLMConfigTest, GLM47FlashConfigParsing) {
    // GLM-4.7-Flash is a 30B-A3B MoE model
    std::string flash_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "model_type": "glm4",
        "hidden_size": 4096,
        "num_hidden_layers": 40,
        "num_attention_heads": 32,
        "num_key_value_heads": 2,
        "intermediate_size": 13696,
        "vocab_size": 151552,
        "max_position_embeddings": 131072,
        "num_experts": 8,
        "num_experts_per_tok": 2,
        "rope_theta": 10000.0,
        "layer_norm_eps": 1e-5
    })";

    auto config = safetensors::glm::parse_glm_config(flash_config);

    EXPECT_EQ(config.hidden_size, 4096);
    EXPECT_EQ(config.num_hidden_layers, 40);
    EXPECT_EQ(config.num_attention_heads, 32);
    EXPECT_EQ(config.num_key_value_heads, 2);
    EXPECT_EQ(config.intermediate_size, 13696);
    EXPECT_EQ(config.vocab_size, 151552);
    EXPECT_EQ(config.max_position_embeddings, 131072);

    // MoE configuration
    EXPECT_TRUE(config.is_moe);
    EXPECT_EQ(config.moe_config.num_experts, 8);
    EXPECT_EQ(config.moe_config.num_experts_per_tok, 2);

    // RoPE configuration
    EXPECT_FLOAT_EQ(config.rope_theta, 10000.0f);
    EXPECT_FLOAT_EQ(config.layer_norm_eps, 1e-5f);
}

// Test: GLM-4.7-FP8 config parsing (FP8 quantization)
TEST_F(GLMConfigTest, GLM47FP8ConfigParsing) {
    // GLM-4.7-FP8 uses FP8 quantization
    // Config with torch_dtype indicating FP8
    std::string fp8_dtype_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "torch_dtype": "float8_e4m3fn"
    })";

    auto config1 = safetensors::glm::parse_glm_config(fp8_dtype_config);
    EXPECT_TRUE(config1.is_fp8);

    // Config with quantization_config
    std::string fp8_quant_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "quantization_config": {
            "quant_method": "fp8"
        }
    })";

    auto config2 = safetensors::glm::parse_glm_config(fp8_quant_config);
    EXPECT_TRUE(config2.is_fp8);

    // Non-FP8 config
    std::string bf16_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "torch_dtype": "bfloat16"
    })";

    auto config3 = safetensors::glm::parse_glm_config(bf16_config);
    EXPECT_FALSE(config3.is_fp8);
}

// Test: GLM model dimensions
TEST_F(GLMConfigTest, ModelDimensions) {
    // Verify model dimensions are correctly parsed
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "intermediate_size": 13696
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);

    EXPECT_EQ(config.hidden_size, 4096);
    EXPECT_EQ(config.num_hidden_layers, 32);
    EXPECT_EQ(config.num_attention_heads, 32);
    EXPECT_EQ(config.intermediate_size, 13696);

    // head_dim should be calculated as hidden_size / num_attention_heads
    EXPECT_EQ(config.head_dim, 128); // 4096 / 32
}

// Test: GLM attention configuration (GQA/MQA)
TEST_F(GLMConfigTest, AttentionConfiguration) {
    // GLM-4.7 uses GQA (Grouped Query Attention)
    // 32 Q heads, 2 KV heads -> group size of 16
    std::string gqa_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "num_key_value_heads": 2
    })";

    auto config = safetensors::glm::parse_glm_config(gqa_config);

    EXPECT_EQ(config.num_attention_heads, 32);
    EXPECT_EQ(config.num_key_value_heads, 2);
    // GQA ratio: 32 / 2 = 16

    // MHA fallback when num_key_value_heads is not specified
    std::string mha_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32
    })";

    auto mha = safetensors::glm::parse_glm_config(mha_config);
    // Should default to MHA (num_key_value_heads == num_attention_heads)
    EXPECT_EQ(mha.num_key_value_heads, mha.num_attention_heads);
}

// Test: GLM RoPE (Rotary Position Embedding) configuration
TEST_F(GLMConfigTest, RoPEConfiguration) {
    // GLM uses RoPE for position encoding
    // Standard rope_theta config
    std::string rope_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "rope_theta": 10000.0
    })";

    auto config = safetensors::glm::parse_glm_config(rope_config);
    EXPECT_FLOAT_EQ(config.rope_theta, 10000.0f);

    // Alternative key name (rotary_emb_base)
    std::string rotary_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "rotary_emb_base": 500000.0
    })";

    auto config2 = safetensors::glm::parse_glm_config(rotary_config);
    EXPECT_FLOAT_EQ(config2.rope_theta, 500000.0f);
}

// Test: GLM vocabulary and tokenizer configuration
TEST_F(GLMConfigTest, VocabularyConfiguration) {
    std::string vocab_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "vocab_size": 151552,
        "bos_token_id": 1,
        "eos_token_id": 2,
        "pad_token_id": 0
    })";

    auto config = safetensors::glm::parse_glm_config(vocab_config);

    EXPECT_EQ(config.vocab_size, 151552);
    EXPECT_EQ(config.bos_token_id, 1);
    EXPECT_EQ(config.eos_token_id, 2);
    EXPECT_EQ(config.pad_token_id, 0);

    // Test with array of eos_token_ids (some models have multiple)
    std::string multi_eos_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "vocab_size": 151552,
        "eos_token_id": [2, 151329, 151336, 151338]
    })";

    auto config2 = safetensors::glm::parse_glm_config(multi_eos_config);
    EXPECT_EQ(config2.eos_token_id, 2); // First EOS token
}

// Test: GLM context length configuration
TEST_F(GLMConfigTest, ContextLengthConfiguration) {
    // Standard max_position_embeddings
    std::string ctx_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "max_position_embeddings": 131072
    })";

    auto config = safetensors::glm::parse_glm_config(ctx_config);
    EXPECT_EQ(config.max_position_embeddings, 131072);

    // Alternative key name (seq_length)
    std::string seq_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "seq_length": 8192
    })";

    auto config2 = safetensors::glm::parse_glm_config(seq_config);
    EXPECT_EQ(config2.max_position_embeddings, 8192);
}

// Test: GLM layer norm configuration
TEST_F(GLMConfigTest, LayerNormConfiguration) {
    // Standard layer_norm_eps
    std::string ln_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "layer_norm_eps": 1e-5,
        "use_rms_norm": true
    })";

    auto config = safetensors::glm::parse_glm_config(ln_config);
    EXPECT_FLOAT_EQ(config.layer_norm_eps, 1e-5f);
    EXPECT_TRUE(config.use_rms_norm);

    // Alternative key name (layernorm_epsilon)
    std::string ln_alt_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "layernorm_epsilon": 1e-6
    })";

    auto config2 = safetensors::glm::parse_glm_config(ln_alt_config);
    EXPECT_FLOAT_EQ(config2.layer_norm_eps, 1e-6f);
}

// Test: Invalid config.json handling
TEST_F(GLMConfigTest, InvalidConfigHandling) {
    // Invalid JSON should throw
    std::string invalid_json = "not valid json {{{";
    EXPECT_THROW(
        safetensors::glm::parse_glm_config(invalid_json),
        std::runtime_error
    );

    // Valid JSON but minimal config should use defaults
    std::string minimal_config = R"({
        "architectures": ["GLM4ForCausalLM"]
    })";

    auto config = safetensors::glm::parse_glm_config(minimal_config);
    // Check defaults are applied
    EXPECT_EQ(config.hidden_size, 4096);      // Default
    EXPECT_EQ(config.num_hidden_layers, 32);  // Default
    EXPECT_FALSE(config.is_moe);              // Default: not MoE
    EXPECT_FALSE(config.is_fp8);              // Default: not FP8
}

// Test: GLM tensor name mapping
TEST_F(GLMConfigTest, TensorNameMapping) {
    // Test extract_layer_index function
    // GLM format: "transformer.encoder.layers.N.*"
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.layers.0.self_attention.query_key_value.weight"), 0);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.layers.15.mlp.dense_h_to_4h.weight"), 15);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.layers.31.input_layernorm.weight"), 31);

    // Alternative format: "model.layers.N.*"
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "model.layers.0.self_attn.q_proj.weight"), 0);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "model.layers.23.mlp.gate_proj.weight"), 23);

    // Non-layer tensors should return -1
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.embedding.word_embeddings.weight"), -1);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "transformer.encoder.final_layernorm.weight"), -1);
    EXPECT_EQ(safetensors::glm::extract_layer_index(
        "lm_head.weight"), -1);
}

// Test: GLM special tokens configuration
TEST_F(GLMConfigTest, SpecialTokensConfiguration) {
    // GLM has specific special tokens
    std::string tokens_config = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32,
        "bos_token_id": 1,
        "eos_token_id": 2,
        "pad_token_id": 0
    })";

    auto config = safetensors::glm::parse_glm_config(tokens_config);

    EXPECT_EQ(config.bos_token_id, 1);
    EXPECT_EQ(config.eos_token_id, 2);
    EXPECT_EQ(config.pad_token_id, 0);

    // GLM-4.7 supports Interleaved Thinking
    EXPECT_TRUE(config.supports_thinking);
}
