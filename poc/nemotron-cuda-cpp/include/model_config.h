#pragma once

#include <string>
#include <vector>
#include <algorithm>
#include "config.h"

namespace nemotron {

// Model configuration from config.json
struct ModelConfig {
    // Architecture
    std::string model_type = "nemotron";
    size_t hidden_size = 4096;
    size_t intermediate_size = 14336;
    size_t num_attention_heads = 32;
    size_t num_hidden_layers = 32;
    size_t num_key_value_heads = 8;  // GQA
    size_t vocab_size = 256000;
    size_t max_position_embeddings = 4096;

    // Inference limit (to avoid OOM with large max_position_embeddings)
    static constexpr size_t MAX_INFERENCE_SEQ_LEN = 4096;
    size_t getMaxSeqLen() const {
        return std::min(max_position_embeddings, MAX_INFERENCE_SEQ_LEN);
    }

    // Normalization
    float rms_norm_eps = 1e-5f;

    // RoPE
    float rope_theta = 10000.0f;

    // Derived values (computed after loading)
    size_t head_dim() const { return hidden_size / num_attention_heads; }
    size_t kv_heads() const { return num_key_value_heads; }
    size_t q_heads() const { return num_attention_heads; }

    // Validate configuration
    bool validate(std::string& error) const;
};

// Load config from JSON file
ModelConfig loadModelConfig(const std::string& config_path);

// Get tensor name patterns for Nemotron
struct TensorNames {
    static std::string embed_tokens() { return "model.embed_tokens.weight"; }
    static std::string lm_head() { return "lm_head.weight"; }
    static std::string final_norm() { return "model.norm.weight"; }

    static std::string layer_input_norm(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".input_layernorm.weight";
    }
    static std::string layer_post_attn_norm(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".post_attention_layernorm.weight";
    }
    static std::string q_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".self_attn.q_proj.weight";
    }
    static std::string k_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".self_attn.k_proj.weight";
    }
    static std::string v_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".self_attn.v_proj.weight";
    }
    static std::string o_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".self_attn.o_proj.weight";
    }
    static std::string gate_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".mlp.gate_proj.weight";
    }
    static std::string up_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".mlp.up_proj.weight";
    }
    static std::string down_proj(size_t layer) {
        return "model.layers." + std::to_string(layer) + ".mlp.down_proj.weight";
    }
};

}  // namespace nemotron
