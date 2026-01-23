#include "nemotron3.h"
#include <nlohmann/json.hpp>
#include <stdexcept>
#include <algorithm>

using json = nlohmann::json;

namespace safetensors {
namespace nemotron3 {

// Layer Type Detection

LayerType get_layer_type(int layer_idx, const Nemotron3Config& config) {
    // Check if layer is in MoE indices
    if (std::find(config.moe_layer_indices.begin(), config.moe_layer_indices.end(), layer_idx)
        != config.moe_layer_indices.end()) {
        return LayerType::MOE;
    }

    // Check if layer is in Mamba indices
    if (std::find(config.mamba_layer_indices.begin(), config.mamba_layer_indices.end(), layer_idx)
        != config.mamba_layer_indices.end()) {
        return LayerType::MAMBA2;
    }

    // Check if layer is in GQA indices
    if (std::find(config.gqa_layer_indices.begin(), config.gqa_layer_indices.end(), layer_idx)
        != config.gqa_layer_indices.end()) {
        return LayerType::GQA;
    }

    throw std::runtime_error("Unknown layer type for layer " + std::to_string(layer_idx));
}

// Config Parsing

Nemotron3Config parse_nemotron3_config(const std::string& config_json) {
    Nemotron3Config config{};

    try {
        json j = json::parse(config_json);

        // Basic model dimensions
        config.d_model = j.value("hidden_size", 2560);
        config.n_layers = j.value("num_hidden_layers", 52);
        config.vocab_size = j.value("vocab_size", 32000);
        config.max_seq_len = j.value("max_position_embeddings", 1048576);

        // MoE config
        if (j.contains("moe_config")) {
            auto moe = j["moe_config"];
            config.n_routed_experts = moe.value("num_experts", 128);
            config.n_shared_experts = moe.value("num_shared_experts", 2);
            config.moe_top_k = moe.value("num_experts_per_tok", 6);
        } else {
            // Defaults
            config.n_routed_experts = 128;
            config.n_shared_experts = 2;
            config.moe_top_k = 6;
        }

        // Mamba config
        if (j.contains("mamba_config")) {
            auto mamba = j["mamba_config"];
            config.mamba_d_state = mamba.value("state_size", 64);
            config.mamba_d_conv = mamba.value("conv_kernel_size", 4);
        } else {
            // Defaults
            config.mamba_d_state = 64;
            config.mamba_d_conv = 4;
        }

        // GQA config
        config.n_heads = j.value("num_attention_heads", 32);
        config.n_kv_heads = j.value("num_key_value_heads", 8);

        // Layer composition
        // Nemotron 3 Nano: 23 MoE + 23 Mamba + 6 GQA = 52 layers
        config.n_moe_layers = 23;
        config.n_mamba_layers = 23;
        config.n_gqa_layers = 6;

        // Layer indices (interleaved pattern)
        // MoE and Mamba alternate, with GQA at specific positions
        for (int i = 0; i < config.n_layers; ++i) {
            // GQA layers at specific positions (e.g., every 8-9 layers)
            if (i % 9 == 0 && config.gqa_layer_indices.size() < config.n_gqa_layers) {
                config.gqa_layer_indices.push_back(i);
            }
            // Alternate MoE and Mamba
            else if ((i % 2) == 0 && config.moe_layer_indices.size() < config.n_moe_layers) {
                config.moe_layer_indices.push_back(i);
            }
            else if (config.mamba_layer_indices.size() < config.n_mamba_layers) {
                config.mamba_layer_indices.push_back(i);
            }
        }

    } catch (const json::exception& e) {
        throw std::runtime_error("Failed to parse Nemotron 3 config: " + std::string(e.what()));
    }

    return config;
}

// Single Layer Forward Pass

struct ggml_tensor* nemotron3_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    int layer_idx,
    const Nemotron3Weights& weights,
    const Nemotron3Config& config,
    MambaState* mamba_state) {

    LayerType layer_type = get_layer_type(layer_idx, config);

    switch (layer_type) {
        case LayerType::MOE: {
            // Find MoE layer index in moe_layers vector
            auto it = std::find(config.moe_layer_indices.begin(), config.moe_layer_indices.end(), layer_idx);
            if (it == config.moe_layer_indices.end()) {
                throw std::runtime_error("MoE layer index not found");
            }
            int moe_idx = std::distance(config.moe_layer_indices.begin(), it);

            MoELayerConfig moe_config{};
            moe_config.d_model = config.d_model;
            moe_config.d_ff = config.d_model * 4;  // Typical expansion
            moe_config.n_routed_experts = config.n_routed_experts;
            moe_config.n_shared_experts = config.n_shared_experts;
            moe_config.top_k = config.moe_top_k;
            moe_config.router_jitter = 0.01f;

            return moe_layer_forward(ctx, input, weights.moe_layers[moe_idx], moe_config);
        }

        case LayerType::MAMBA2: {
            // Find Mamba layer index in mamba_layers vector
            auto it = std::find(config.mamba_layer_indices.begin(), config.mamba_layer_indices.end(), layer_idx);
            if (it == config.mamba_layer_indices.end()) {
                throw std::runtime_error("Mamba layer index not found");
            }
            int mamba_idx = std::distance(config.mamba_layer_indices.begin(), it);

            MambaLayerConfig mamba_config{};
            mamba_config.d_model = config.d_model;
            mamba_config.d_inner = config.d_model * 2;  // 2x expansion
            mamba_config.d_state = config.mamba_d_state;
            mamba_config.conv_kernel_size = config.mamba_d_conv;
            mamba_config.dt_rank = 32.0f;
            mamba_config.dt_min = 0.001f;
            mamba_config.dt_max = 0.1f;

            return mamba_layer_forward(ctx, input, weights.mamba_layers[mamba_idx], mamba_state, mamba_config);
        }

        case LayerType::GQA: {
            // Find GQA layer index in gqa_layers vector
            auto it = std::find(config.gqa_layer_indices.begin(), config.gqa_layer_indices.end(), layer_idx);
            if (it == config.gqa_layer_indices.end()) {
                throw std::runtime_error("GQA layer index not found");
            }
            int gqa_idx = std::distance(config.gqa_layer_indices.begin(), it);

            GQALayerConfig gqa_config{};
            gqa_config.d_model = config.d_model;
            gqa_config.n_heads = config.n_heads;
            gqa_config.n_kv_groups = 2;  // Nemotron 3 uses 2 KV groups
            gqa_config.head_dim = config.d_model / config.n_heads;
            gqa_config.max_seq_len = config.max_seq_len;

            // Note: KV cache management would be added for autoregressive generation
            // For now, pass nullptr for kv_cache
            return gqa_layer_forward(ctx, input, weights.gqa_layers[gqa_idx], nullptr, gqa_config, 0);
        }

        default:
            throw std::runtime_error("Unknown layer type");
    }
}

// Full Model Forward Pass

struct ggml_tensor* nemotron3_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input_ids,
    const Nemotron3Weights& weights,
    const Nemotron3Config& config,
    std::vector<MambaState*>* mamba_states) {

    if (!ctx || !input_ids) {
        throw std::invalid_argument("Invalid arguments to nemotron3_forward");
    }

    // 1. Token embedding
    struct ggml_tensor* hidden = ggml_get_rows(ctx, weights.token_embedding, input_ids);
    ggml_set_name(hidden, "token_embedding");

    // 2. Forward pass through all layers
    int mamba_state_idx = 0;
    for (int layer_idx = 0; layer_idx < config.n_layers; ++layer_idx) {
        LayerType layer_type = get_layer_type(layer_idx, config);

        MambaState* mamba_state = nullptr;
        if (layer_type == LayerType::MAMBA2 && mamba_states != nullptr) {
            if (mamba_state_idx < mamba_states->size()) {
                mamba_state = (*mamba_states)[mamba_state_idx];
            }
            mamba_state_idx++;
        }

        hidden = nemotron3_layer_forward(ctx, hidden, layer_idx, weights, config, mamba_state);
        ggml_set_name(hidden, ("layer_" + std::to_string(layer_idx)).c_str());
    }

    // 3. Output normalization
    if (weights.output_norm_weight) {
        hidden = ggml_norm(ctx, hidden, 1e-5f);
        hidden = ggml_mul(ctx, hidden, weights.output_norm_weight);
        if (weights.output_norm_bias) {
            hidden = ggml_add(ctx, hidden, weights.output_norm_bias);
        }
        ggml_set_name(hidden, "output_norm");
    }

    // 4. Language model head
    struct ggml_tensor* logits = ggml_mul_mat(ctx, weights.lm_head_weight, hidden);
    ggml_set_name(logits, "logits");

    return logits;
}

// Weights Loading

Nemotron3Weights load_nemotron3_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const Nemotron3Config& config) {

    Nemotron3Weights weights{};

    auto get_tensor = [&](const std::string& name, bool required = true) -> struct ggml_tensor* {
        auto it = tensors.find(name);
        if (it == tensors.end()) {
            if (required) {
                throw std::runtime_error("Required tensor not found: " + name);
            }
            return nullptr;
        }
        return it->second;
    };

    // Load embedding
    weights.token_embedding = get_tensor("model.embed_tokens.weight");

    // Load MoE layer weights
    for (int i = 0; i < config.n_moe_layers; ++i) {
        int layer_idx = config.moe_layer_indices[i];
        std::string prefix = "model.layers." + std::to_string(layer_idx) + ".moe";

        MoELayerConfig moe_config{};
        moe_config.d_model = config.d_model;
        moe_config.d_ff = config.d_model * 4;
        moe_config.n_routed_experts = config.n_routed_experts;
        moe_config.n_shared_experts = config.n_shared_experts;
        moe_config.top_k = config.moe_top_k;

        weights.moe_layers.push_back(load_moe_layer_weights(ctx, tensors, prefix, moe_config));
    }

    // Load Mamba layer weights
    for (int i = 0; i < config.n_mamba_layers; ++i) {
        int layer_idx = config.mamba_layer_indices[i];
        std::string prefix = "model.layers." + std::to_string(layer_idx) + ".mamba";

        MambaLayerConfig mamba_config{};
        mamba_config.d_model = config.d_model;
        mamba_config.d_inner = config.d_model * 2;
        mamba_config.d_state = config.mamba_d_state;
        mamba_config.conv_kernel_size = config.mamba_d_conv;

        weights.mamba_layers.push_back(load_mamba_layer_weights(ctx, tensors, prefix, mamba_config));
    }

    // Load output weights
    weights.output_norm_weight = get_tensor("model.norm.weight", false);
    weights.output_norm_bias = get_tensor("model.norm.bias", false);
    weights.lm_head_weight = get_tensor("lm_head.weight");

    return weights;
}

} // namespace nemotron3
} // namespace safetensors
