/**
 * @file transformer.cpp
 * @brief Transformer compute graph implementation (Task 27)
 *
 * Builds ggml compute graphs for transformer forward pass including:
 * - RMSNorm
 * - Rotary Position Embeddings (RoPE)
 * - Multi-Head Attention (MHA/GQA/MQA)
 * - SwiGLU FFN
 */

#include "ggml_model.h"
#include <cmath>
#include <cstring>

namespace stcpp {

/* RMSNorm operation */
static struct ggml_tensor* rms_norm(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* weight,
    float eps
) {
    // RMSNorm(x) = x / sqrt(mean(x^2) + eps) * weight
    x = ggml_rms_norm(ctx, x, eps);
    return ggml_mul(ctx, x, weight);
}

/* Apply RoPE to Q and K tensors */
static void apply_rope(
    struct ggml_context* ctx,
    struct ggml_tensor** q,
    struct ggml_tensor** k,
    int32_t n_past,
    int32_t n_rot,
    float freq_base,
    float freq_scale
) {
    // Build RoPE parameters
    // Position indices: n_past to n_past + seq_len - 1
    const int mode = 0;  // Standard RoPE

    *q = ggml_rope_ext(
        ctx, *q, nullptr, nullptr,
        n_rot, mode, 0,
        freq_base, freq_scale, 0.0f, 1.0f, 0.0f, 0.0f
    );

    *k = ggml_rope_ext(
        ctx, *k, nullptr, nullptr,
        n_rot, mode, 0,
        freq_base, freq_scale, 0.0f, 1.0f, 0.0f, 0.0f
    );
}

/* Multi-head attention */
static struct ggml_tensor* multi_head_attention(
    struct ggml_context* ctx,
    struct ggml_tensor* cur,         // Input [n_embd, n_tokens]
    const LayerTensors& layer,
    struct ggml_tensor* k_cache,     // KV cache for keys
    struct ggml_tensor* v_cache,     // KV cache for values
    int32_t n_past,                  // Number of past tokens in KV cache
    int32_t n_tokens,                // Number of current tokens
    int32_t n_head,
    int32_t n_head_kv,
    int32_t n_embd,
    int32_t n_rot,
    float freq_base,
    float freq_scale,
    int32_t layer_idx
) {
    const int32_t head_dim = n_embd / n_head;

    // Q, K, V projections
    struct ggml_tensor* q = ggml_mul_mat(ctx, layer.wq, cur);
    struct ggml_tensor* k = ggml_mul_mat(ctx, layer.wk, cur);
    struct ggml_tensor* v = ggml_mul_mat(ctx, layer.wv, cur);

    // Reshape for multi-head attention
    // Q: [n_embd, n_tokens] -> [head_dim, n_head, n_tokens]
    q = ggml_reshape_3d(ctx, q, head_dim, n_head, n_tokens);
    k = ggml_reshape_3d(ctx, k, head_dim, n_head_kv, n_tokens);
    v = ggml_reshape_3d(ctx, v, head_dim, n_head_kv, n_tokens);

    // Apply RoPE
    apply_rope(ctx, &q, &k, n_past, n_rot, freq_base, freq_scale);

    // Store K, V in cache
    // k_cache shape: [head_dim, n_head_kv, n_ctx, n_layer]
    // We view into the cache for this layer
    struct ggml_tensor* k_cache_layer = ggml_view_3d(
        ctx, k_cache,
        head_dim, n_head_kv, n_tokens,
        k_cache->nb[1], k_cache->nb[2],
        layer_idx * k_cache->nb[3] + n_past * k_cache->nb[2]
    );

    struct ggml_tensor* v_cache_layer = ggml_view_3d(
        ctx, v_cache,
        head_dim, n_head_kv, n_tokens,
        v_cache->nb[1], v_cache->nb[2],
        layer_idx * v_cache->nb[3] + n_past * v_cache->nb[2]
    );

    // Copy current K, V to cache (tensors are added to graph later)
    struct ggml_tensor* k_cpy = ggml_cpy(ctx, k, k_cache_layer);
    struct ggml_tensor* v_cpy = ggml_cpy(ctx, v, v_cache_layer);
    (void)k_cpy;
    (void)v_cpy;

    // Get full K, V from cache (including past)
    const int32_t n_kv = n_past + n_tokens;

    struct ggml_tensor* K = ggml_view_3d(
        ctx, k_cache,
        head_dim, n_head_kv, n_kv,
        k_cache->nb[1], k_cache->nb[2],
        layer_idx * k_cache->nb[3]
    );

    struct ggml_tensor* V = ggml_view_3d(
        ctx, v_cache,
        head_dim, n_head_kv, n_kv,
        v_cache->nb[1], v_cache->nb[2],
        layer_idx * v_cache->nb[3]
    );

    // Handle GQA: repeat K, V heads if needed
    if (n_head_kv < n_head) {
        const int32_t n_rep = n_head / n_head_kv;
        K = ggml_repeat(ctx, K, ggml_new_tensor_3d(ctx, K->type, head_dim, n_head, n_kv));
        V = ggml_repeat(ctx, V, ggml_new_tensor_3d(ctx, V->type, head_dim, n_head, n_kv));
    }

    // Compute attention scores: Q @ K^T
    // Q: [head_dim, n_head, n_tokens]
    // K: [head_dim, n_head, n_kv] -> K^T: [n_kv, n_head, head_dim]
    K = ggml_permute(ctx, K, 0, 2, 1, 3);  // [head_dim, n_kv, n_head]
    struct ggml_tensor* scores = ggml_mul_mat(ctx, K, q);  // [n_kv, n_head, n_tokens]

    // Scale
    const float scale = 1.0f / sqrtf((float)head_dim);
    scores = ggml_scale(ctx, scores, scale);

    // Causal mask
    scores = ggml_diag_mask_inf(ctx, scores, n_past);

    // Softmax
    scores = ggml_soft_max(ctx, scores);

    // Apply attention to values: scores @ V
    // scores: [n_kv, n_head, n_tokens]
    // V: [head_dim, n_head, n_kv]
    V = ggml_permute(ctx, V, 1, 2, 0, 3);  // [n_kv, head_dim, n_head]
    V = ggml_cont(ctx, V);

    struct ggml_tensor* attn_out = ggml_mul_mat(ctx, V, scores);  // [head_dim, n_head, n_tokens]

    // Reshape and output projection
    attn_out = ggml_cont(ctx, ggml_permute(ctx, attn_out, 0, 2, 1, 3));
    attn_out = ggml_reshape_2d(ctx, attn_out, n_embd, n_tokens);
    attn_out = ggml_mul_mat(ctx, layer.wo, attn_out);

    return attn_out;
}

/* SwiGLU FFN */
static struct ggml_tensor* swiglu_ffn(
    struct ggml_context* ctx,
    struct ggml_tensor* cur,
    const LayerTensors& layer
) {
    // SwiGLU: swish(gate(x)) * up(x), then down projection
    // gate(x) = x @ W_gate
    // up(x) = x @ W_up
    // down(swish(gate) * up) = result @ W_down

    struct ggml_tensor* gate = ggml_mul_mat(ctx, layer.ffn_gate, cur);
    struct ggml_tensor* up = ggml_mul_mat(ctx, layer.ffn_up, cur);

    // SiLU (Swish) activation on gate
    gate = ggml_silu(ctx, gate);

    // Element-wise multiply
    struct ggml_tensor* x = ggml_mul(ctx, gate, up);

    // Down projection
    x = ggml_mul_mat(ctx, layer.ffn_down, x);

    return x;
}

/* Build transformer layer */
static struct ggml_tensor* build_layer(
    struct ggml_context* ctx,
    struct ggml_tensor* cur,
    const LayerTensors& layer,
    struct ggml_tensor* k_cache,
    struct ggml_tensor* v_cache,
    int32_t n_past,
    int32_t n_tokens,
    const ModelHParams& hparams,
    int32_t layer_idx
) {
    struct ggml_tensor* residual = cur;

    // Pre-attention RMSNorm
    cur = rms_norm(ctx, cur, layer.attn_norm, hparams.norm_eps);

    // Multi-head attention
    cur = multi_head_attention(
        ctx, cur, layer,
        k_cache, v_cache,
        n_past, n_tokens,
        hparams.n_head, hparams.n_head_kv,
        hparams.n_embd, hparams.n_rot,
        hparams.rope_freq_base, hparams.rope_freq_scale,
        layer_idx
    );

    // Residual connection
    cur = ggml_add(ctx, cur, residual);
    residual = cur;

    // Pre-FFN RMSNorm
    cur = rms_norm(ctx, cur, layer.ffn_norm, hparams.norm_eps);

    // FFN
    cur = swiglu_ffn(ctx, cur, layer);

    // Residual connection
    cur = ggml_add(ctx, cur, residual);

    return cur;
}

/* Build full compute graph for forward pass */
struct ggml_cgraph* build_compute_graph(
    GgmlContext* ctx,
    const int32_t* tokens,
    int32_t n_tokens,
    int32_t n_past
) {
    GgmlModel* model = ctx->model;
    const ModelHParams& hparams = model->hparams;
    const ModelTensors& tensors = model->tensors;

    // Estimate buffer size for compute
    size_t compute_size = estimate_compute_buffer_size(hparams, ctx->kv_size, n_tokens);

    struct ggml_init_params graph_params = {
        .mem_size = compute_size,
        .mem_buffer = nullptr,
        .no_alloc = false,
    };

    struct ggml_context* ctx_graph = ggml_init(graph_params);
    if (!ctx_graph) {
        return nullptr;
    }

    // Create token input tensor
    struct ggml_tensor* inp_tokens = ggml_new_tensor_1d(ctx_graph, GGML_TYPE_I32, n_tokens);
    ggml_set_name(inp_tokens, "inp_tokens");
    memcpy(inp_tokens->data, tokens, n_tokens * sizeof(int32_t));

    // Token embeddings lookup
    struct ggml_tensor* cur = ggml_get_rows(ctx_graph, tensors.tok_embd, inp_tokens);

    // Reshape to [n_embd, n_tokens]
    cur = ggml_reshape_2d(ctx_graph, cur, hparams.n_embd, n_tokens);

    // Process each transformer layer
    for (int i = 0; i < hparams.n_layer; ++i) {
        cur = build_layer(
            ctx_graph, cur,
            tensors.layers[i],
            ctx->k_cache, ctx->v_cache,
            n_past, n_tokens,
            hparams, i
        );
    }

    // Final RMSNorm
    cur = rms_norm(ctx_graph, cur, tensors.output_norm, hparams.norm_eps);

    // LM head
    cur = ggml_mul_mat(ctx_graph, tensors.output, cur);

    ggml_set_name(cur, "logits");

    // Build graph
    struct ggml_cgraph* graph = ggml_new_graph(ctx_graph);
    ggml_build_forward_expand(graph, cur);

    return graph;
}

/* Run forward pass */
bool forward_pass(
    GgmlContext* ctx,
    const int32_t* tokens,
    int32_t n_tokens,
    int32_t n_past,
    float* logits,
    std::string& error
) {
    if (!ctx || !ctx->model) {
        error = "Invalid context";
        return false;
    }

    // Check cancel flag
    if (ctx->cancel_flag.load(std::memory_order_acquire)) {
        error = "Cancelled";
        return false;
    }

    // Build compute graph
    struct ggml_cgraph* graph = build_compute_graph(ctx, tokens, n_tokens, n_past);
    if (!graph) {
        error = "Failed to build compute graph";
        return false;
    }

    // Get backend and run
    ggml_backend_t backend = ctx->model->backend;

    // Create a compute plan
    ggml_backend_graph_compute(backend, graph);

    // Extract logits tensor by name
    struct ggml_tensor* logits_tensor = ggml_graph_get_tensor(graph, "logits");

    // Fallback: try last node in graph
    if (!logits_tensor) {
        int n_nodes = ggml_graph_n_nodes(graph);
        if (n_nodes > 0) {
            logits_tensor = ggml_graph_node(graph, n_nodes - 1);
        }
    }

    if (!logits_tensor) {
        error = "Logits tensor not found";
        return false;
    }

    // Copy logits for the last token
    const ModelHParams& hparams = ctx->model->hparams;
    size_t logits_size = hparams.n_vocab * sizeof(float);

    // Get logits for last token position
    size_t offset = (n_tokens - 1) * hparams.n_vocab * sizeof(float);
    ggml_backend_tensor_get(logits_tensor, logits, offset, logits_size);

    // Update KV cache position
    ctx->kv_used = n_past + n_tokens;

    return true;
}

}  // namespace stcpp
