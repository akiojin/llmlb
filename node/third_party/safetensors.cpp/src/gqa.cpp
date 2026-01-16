#include "gqa.h"
#include <cmath>
#include <stdexcept>

namespace safetensors {

// KV Cache Constructor
GQAKVCache::GQAKVCache(struct ggml_context* ctx, const GQALayerConfig& config)
    : current_seq_len(0) {

    if (!ctx) {
        throw std::runtime_error("GQAKVCache: null context");
    }

    int kv_dim = config.n_kv_groups * config.head_dim;

    // Allocate KV cache tensors
    k_cache = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, kv_dim, config.max_seq_len);
    v_cache = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, kv_dim, config.max_seq_len);

    if (!k_cache || !v_cache) {
        throw std::runtime_error("Failed to allocate KV cache tensors");
    }

    ggml_set_name(k_cache, "gqa_k_cache");
    ggml_set_name(v_cache, "gqa_v_cache");

    reset();
}

void GQAKVCache::reset() {
    current_seq_len = 0;
    // Note: In a real implementation, you'd zero out the cache tensors here
    // For now, we rely on ggml's tensor initialization
}

void GQAKVCache::append(struct ggml_tensor* k, struct ggml_tensor* v, int seq_len) {
    if (!k || !v) {
        throw std::runtime_error("GQAKVCache::append: null tensors");
    }

    if (!k_cache || !v_cache) {
        throw std::runtime_error("GQAKVCache::append: cache not initialized");
    }

    // Check bounds
    if (current_seq_len + seq_len > k_cache->ne[1]) {
        throw std::runtime_error("GQAKVCache::append: cache overflow");
    }

    // Get ggml context from tensors (for creating view/cpy operations)
    // Note: In production code, context should be passed as parameter
    // For now, we update current_seq_len only
    // Actual copying will be handled during graph execution via ggml_build_forward_expand

    // Create views into cache at current position
    // k_cache: [kv_dim, max_seq_len] -> view at [kv_dim, current_seq_len:current_seq_len+seq_len]
    // This requires ggml context to create view and cpy operations
    // These operations will be added to the computation graph

    // For autoregressive generation, the typical pattern is:
    // 1. Compute new K,V for current token
    // 2. Append to cache
    // 3. Use full cache for attention

    // Update sequence length
    current_seq_len += seq_len;

    // Note: Actual tensor copying should be done via ggml_cpy in the computation graph
    // This method primarily updates bookkeeping
}

// Apply RoPE to Query/Key tensors
struct ggml_tensor* apply_rope_gqa(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    int n_past,
    int n_rot,
    int mode,
    int n_ctx,
    float rope_theta,
    float rope_freq_scale) {

    if (!ctx || !x) {
        throw std::runtime_error("apply_rope_gqa: null context or tensor");
    }

    // Use ggml_rope for RoPE application
    // mode: 0 = standard RoPE, 2 = neox-style RoPE
    struct ggml_tensor* rope_x = ggml_rope(ctx, x, n_past, n_rot, mode, n_ctx);

    if (!rope_x) {
        throw std::runtime_error("apply_rope_gqa: ggml_rope failed");
    }

    return rope_x;
}

// Reshape tensor to multi-head format
static struct ggml_tensor* reshape_to_heads(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    int batch_size,
    int seq_len,
    int n_heads,
    int head_dim) {

    // x: [batch * seq_len, n_heads * head_dim]
    // -> [batch, seq_len, n_heads, head_dim]
    // -> permute to [batch, n_heads, seq_len, head_dim] for attention

    struct ggml_tensor* x_4d = ggml_reshape_4d(ctx, x, head_dim, n_heads, seq_len, batch_size);
    struct ggml_tensor* x_perm = ggml_permute(ctx, x_4d, 0, 2, 1, 3);  // [batch, n_heads, seq_len, head_dim]

    return x_perm;
}

// Repeat KV groups to match number of query heads
static struct ggml_tensor* repeat_kv_groups(
    struct ggml_context* ctx,
    struct ggml_tensor* kv,  // [batch, n_kv_groups, seq_len_kv, head_dim]
    int n_heads,
    int n_kv_groups) {

    if (!ctx || !kv) {
        throw std::runtime_error("repeat_kv_groups: null inputs");
    }

    // Each KV group needs to be repeated n_heads / n_kv_groups times
    // e.g., for 32 heads and 2 groups: each group is repeated 16 times
    int repeat_factor = n_heads / n_kv_groups;

    if (repeat_factor == 1) {
        // No repetition needed (MHA case)
        return kv;
    }

    // Get current dimensions: [batch, n_kv_groups, seq_len_kv, head_dim]
    int batch = kv->ne[3];
    int n_kv = kv->ne[2];
    int seq_len_kv = kv->ne[1];
    int head_dim = kv->ne[0];

    // Create output tensor: [batch, n_heads, seq_len_kv, head_dim]
    struct ggml_tensor* kv_repeated = ggml_new_tensor_4d(ctx, kv->type,
        head_dim, seq_len_kv, n_heads, batch);

    if (!kv_repeated) {
        throw std::runtime_error("Failed to allocate repeated KV tensor");
    }

    // Use ggml_repeat to replicate along the n_heads dimension
    // ggml_repeat requires target shape
    kv_repeated = ggml_repeat(ctx, kv, kv_repeated);

    return kv_repeated;
}

// Compute GQA attention
struct ggml_tensor* gqa_attention(
    struct ggml_context* ctx,
    struct ggml_tensor* q,          // [batch, n_heads, seq_len, head_dim]
    struct ggml_tensor* k,          // [batch, n_kv_groups, seq_len_kv, head_dim]
    struct ggml_tensor* v,          // [batch, n_kv_groups, seq_len_kv, head_dim]
    const GQALayerConfig& config,
    bool use_causal_mask) {

    if (!ctx || !q || !k || !v) {
        throw std::runtime_error("gqa_attention: null inputs");
    }

    // Repeat KV to match query heads
    struct ggml_tensor* k_repeated = repeat_kv_groups(ctx, k, config.n_heads, config.n_kv_groups);
    struct ggml_tensor* v_repeated = repeat_kv_groups(ctx, v, config.n_heads, config.n_kv_groups);

    // Scaled dot-product attention
    // scores = (Q @ K^T) / sqrt(head_dim)

    // Q @ K^T
    struct ggml_tensor* k_transposed = ggml_cont(ctx, ggml_permute(ctx, k_repeated, 0, 1, 3, 2));
    struct ggml_tensor* scores = ggml_mul_mat(ctx, k_transposed, q);  // Note: ggml_mul_mat has specific dimension requirements

    // Scale by 1/sqrt(head_dim)
    float scale = 1.0f / sqrtf(static_cast<float>(config.head_dim));
    struct ggml_tensor* scaled_scores = ggml_scale(ctx, scores, scale);

    // Apply causal mask if needed
    if (use_causal_mask) {
        scaled_scores = ggml_diag_mask_inf(ctx, scaled_scores, 0);
    }

    // Softmax
    struct ggml_tensor* attn_weights = ggml_soft_max(ctx, scaled_scores);

    // Attention output: attn_weights @ V
    struct ggml_tensor* attn_output = ggml_mul_mat(ctx, v_repeated, attn_weights);

    return attn_output;
}

// GQA Layer Forward Pass
struct ggml_tensor* gqa_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const GQALayerWeights& weights,
    GQAKVCache* kv_cache,
    const GQALayerConfig& config,
    int n_past) {

    if (!ctx || !input) {
        throw std::runtime_error("gqa_layer_forward: null inputs");
    }

    // 1. Layer normalization (pre-norm)
    struct ggml_tensor* x = input;
    if (weights.norm_weight) {
        x = ggml_norm(ctx, x, 1e-5f);
        x = ggml_mul(ctx, x, weights.norm_weight);
        if (weights.norm_bias) {
            x = ggml_add(ctx, x, weights.norm_bias);
        }
    }

    // Get input dimensions
    // Assuming input shape: [batch * seq_len, d_model]
    int batch_seq = x->ne[1];
    int seq_len = 1;  // For now, assume single token (generation mode)
    int batch_size = batch_seq / seq_len;

    // 2. Query projection (multi-head)
    struct ggml_tensor* q = ggml_mul_mat(ctx, weights.q_proj_weight, x);
    if (weights.q_proj_bias) {
        q = ggml_add(ctx, q, weights.q_proj_bias);
    }

    // 3. Key/Value projections (grouped)
    struct ggml_tensor* k = ggml_mul_mat(ctx, weights.k_proj_weight, x);
    if (weights.k_proj_bias) {
        k = ggml_add(ctx, k, weights.k_proj_bias);
    }

    struct ggml_tensor* v = ggml_mul_mat(ctx, weights.v_proj_weight, x);
    if (weights.v_proj_bias) {
        v = ggml_add(ctx, v, weights.v_proj_bias);
    }

    // 4. Reshape to multi-head format
    q = reshape_to_heads(ctx, q, batch_size, seq_len, config.n_heads, config.head_dim);
    k = reshape_to_heads(ctx, k, batch_size, seq_len, config.n_kv_groups, config.head_dim);
    v = reshape_to_heads(ctx, v, batch_size, seq_len, config.n_kv_groups, config.head_dim);

    // 5. Apply RoPE to Q and K
    int n_rot = config.head_dim;  // Full head dimension rotation
    q = apply_rope_gqa(ctx, q, n_past, n_rot, 0, config.max_seq_len, config.rope_theta, config.rope_freq_scale);
    k = apply_rope_gqa(ctx, k, n_past, n_rot, 0, config.max_seq_len, config.rope_theta, config.rope_freq_scale);

    // 6. Update KV cache
    if (kv_cache) {
        kv_cache->append(k, v, seq_len);
        // In generation mode, use cached K and V for attention
        // This is a placeholder; real implementation needs to concatenate cached KV
    }

    // 7. Compute attention
    struct ggml_tensor* attn_output = gqa_attention(ctx, q, k, v, config, true);

    // 8. Reshape attention output back to [batch * seq_len, d_model]
    attn_output = ggml_cont(ctx, ggml_permute(ctx, attn_output, 0, 2, 1, 3));
    attn_output = ggml_reshape_2d(ctx, attn_output, config.d_model, batch_seq);

    // 9. Output projection
    struct ggml_tensor* output = ggml_mul_mat(ctx, weights.o_proj_weight, attn_output);
    if (weights.o_proj_bias) {
        output = ggml_add(ctx, output, weights.o_proj_bias);
    }

    // 10. Residual connection
    output = ggml_add(ctx, output, input);

    return output;
}

}  // namespace safetensors
