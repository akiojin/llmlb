#pragma once

#include <ggml.h>
#include <vector>

namespace safetensors {

// GQA (Grouped Query Attention) Configuration
struct GQALayerConfig {
    int d_model;            // Model dimension (e.g., 2560)
    int n_heads;            // Number of query heads (e.g., 32)
    int n_kv_groups;        // Number of key/value groups (e.g., 2 for Nemotron 3)
    int head_dim;           // Head dimension (d_model / n_heads)
    int max_seq_len;        // Maximum sequence length

    float rope_theta;       // RoPE theta (e.g., 10000.0)
    float rope_freq_scale;  // RoPE frequency scale (e.g., 1.0)

    float attn_dropout;     // Attention dropout rate (0.0 for inference)

    GQALayerConfig()
        : d_model(2560)
        , n_heads(32)
        , n_kv_groups(2)
        , head_dim(80)  // d_model / n_heads = 2560 / 32
        , max_seq_len(1048576)  // 1M tokens
        , rope_theta(10000.0f)
        , rope_freq_scale(1.0f)
        , attn_dropout(0.0f)
    {}
};

// GQA Layer Weights
struct GQALayerWeights {
    // Pre-attention normalization
    struct ggml_tensor* norm_weight;      // [d_model]
    struct ggml_tensor* norm_bias;        // [d_model] (optional)

    // Query projection (multi-head)
    struct ggml_tensor* q_proj_weight;    // [d_model, d_model]
    struct ggml_tensor* q_proj_bias;      // [d_model] (optional)

    // Key/Value projections (grouped)
    struct ggml_tensor* k_proj_weight;    // [d_model, n_kv_groups * head_dim]
    struct ggml_tensor* k_proj_bias;      // [n_kv_groups * head_dim] (optional)

    struct ggml_tensor* v_proj_weight;    // [d_model, n_kv_groups * head_dim]
    struct ggml_tensor* v_proj_bias;      // [n_kv_groups * head_dim] (optional)

    // Output projection
    struct ggml_tensor* o_proj_weight;    // [d_model, d_model]
    struct ggml_tensor* o_proj_bias;      // [d_model] (optional)
};

// KV Cache for GQA Layer
struct GQAKVCache {
    struct ggml_tensor* k_cache;  // [max_seq_len, n_kv_groups * head_dim]
    struct ggml_tensor* v_cache;  // [max_seq_len, n_kv_groups * head_dim]
    int current_seq_len;          // Current sequence length in cache

    GQAKVCache(struct ggml_context* ctx, const GQALayerConfig& config);

    void reset();
    void append(struct ggml_tensor* k, struct ggml_tensor* v, int seq_len);
};

// Apply RoPE (Rotary Position Embedding) to Query and Key
struct ggml_tensor* apply_rope_gqa(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    int n_past,
    int n_rot,
    int mode,
    int n_ctx,
    float rope_theta,
    float rope_freq_scale);

// GQA Layer Forward Pass
struct ggml_tensor* gqa_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const GQALayerWeights& weights,
    GQAKVCache* kv_cache,
    const GQALayerConfig& config,
    int n_past = 0);

// Compute GQA attention scores
struct ggml_tensor* gqa_attention(
    struct ggml_context* ctx,
    struct ggml_tensor* q,          // [batch, n_heads, seq_len, head_dim]
    struct ggml_tensor* k,          // [batch, n_kv_groups, seq_len_kv, head_dim]
    struct ggml_tensor* v,          // [batch, n_kv_groups, seq_len_kv, head_dim]
    const GQALayerConfig& config,
    bool use_causal_mask = true);

}  // namespace safetensors
