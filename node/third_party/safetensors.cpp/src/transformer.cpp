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

/* Structure to track positions tensors for delayed initialization */
struct PositionsTensorInfo {
    struct ggml_tensor* tensor;
    int32_t n_past;
    int32_t n_tokens;
};

/* Thread-local storage for positions tensors (avoids global initialization issues in plugins) */
struct PositionsStorage {
    PositionsTensorInfo tensors[64];  // Max 64 layers
    int count;
};

static PositionsStorage& get_positions_storage() {
    static thread_local PositionsStorage storage = {{}, 0};
    return storage;
}

/* Apply RoPE to Q and K tensors - creates positions tensor without setting data */
static void apply_rope(
    struct ggml_context* ctx,
    struct ggml_tensor** q,
    struct ggml_tensor** k,
    int32_t n_past,
    int32_t n_tokens,
    int32_t n_rot,
    float freq_base,
    float freq_scale,
    int32_t layer_idx
) {
    // Build position tensor [n_past, n_past+1, ..., n_past+n_tokens-1]
    // Data will be set later after graph allocation
    char positions_name[64];
    snprintf(positions_name, sizeof(positions_name), "positions_%d", layer_idx);

    struct ggml_tensor* positions = ggml_new_tensor_1d(ctx, GGML_TYPE_I32, n_tokens);
    ggml_set_name(positions, positions_name);
    ggml_set_input(positions);  // Mark as input so allocator will allocate it

    // Track this positions tensor for later data initialization
    auto& storage = get_positions_storage();
    if (storage.count < 64) {
        storage.tensors[storage.count].tensor = positions;
        storage.tensors[storage.count].n_past = n_past;
        storage.tensors[storage.count].n_tokens = n_tokens;
        storage.count++;
    }

    const int mode = 0;  // Standard RoPE

    *q = ggml_rope_ext(
        ctx, *q, positions, nullptr,
        n_rot, mode, 0,
        freq_base, freq_scale, 0.0f, 1.0f, 0.0f, 0.0f
    );

    *k = ggml_rope_ext(
        ctx, *k, positions, nullptr,
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
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: entered, n_head=%d, n_head_kv=%d, n_embd=%d, n_rot=%d\n",
                n_head, n_head_kv, n_embd, n_rot);
        fflush(stderr);
    }

    const int32_t head_dim = n_embd / n_head;

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: head_dim=%d, computing Q,K,V projections\n", head_dim);
        fflush(stderr);
    }

    // Q, K, V projections
    struct ggml_tensor* q = ggml_mul_mat(ctx, layer.wq, cur);
    struct ggml_tensor* k = ggml_mul_mat(ctx, layer.wk, cur);
    struct ggml_tensor* v = ggml_mul_mat(ctx, layer.wv, cur);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: Q,K,V projected, reshaping\n");
        fflush(stderr);
    }

    // Reshape for multi-head attention
    // Q: [n_embd, n_tokens] -> [head_dim, n_head, n_tokens]
    q = ggml_reshape_3d(ctx, q, head_dim, n_head, n_tokens);
    k = ggml_reshape_3d(ctx, k, head_dim, n_head_kv, n_tokens);
    v = ggml_reshape_3d(ctx, v, head_dim, n_head_kv, n_tokens);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: reshaped, applying RoPE\n");
        fprintf(stderr, "[DEBUG] MHA[0]: calling apply_rope with ctx=%p, q=%p, k=%p\n",
                (void*)ctx, (void*)q, (void*)k);
        fflush(stderr);
    }

    // Apply RoPE
    apply_rope(ctx, &q, &k, n_past, n_tokens, n_rot, freq_base, freq_scale, layer_idx);
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: apply_rope returned\n");
        fflush(stderr);
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: RoPE applied\n");
        fflush(stderr);
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: storing K,V in cache\n");
        fprintf(stderr, "[DEBUG] MHA[0]: k_cache dims=[%lld,%lld,%lld,%lld], nb=[%zu,%zu,%zu,%zu]\n",
                (long long)k_cache->ne[0], (long long)k_cache->ne[1],
                (long long)k_cache->ne[2], (long long)k_cache->ne[3],
                k_cache->nb[0], k_cache->nb[1], k_cache->nb[2], k_cache->nb[3]);
        fflush(stderr);
    }

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

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: cache views created, copying K,V\n");
        fflush(stderr);
    }

    // Copy current K, V to cache (tensors are added to graph later)
    struct ggml_tensor* k_cpy = ggml_cpy(ctx, k, k_cache_layer);
    struct ggml_tensor* v_cpy = ggml_cpy(ctx, v, v_cache_layer);
    (void)k_cpy;
    (void)v_cpy;

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: K,V copied, getting full cache view\n");
        fflush(stderr);
    }

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

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: handling GQA (n_head=%d, n_head_kv=%d)\n", n_head, n_head_kv);
        fflush(stderr);
    }

    // Handle GQA: repeat K, V heads if needed
    if (n_head_kv < n_head) {
        const int32_t n_rep = n_head / n_head_kv;
        (void)n_rep;
        K = ggml_repeat(ctx, K, ggml_new_tensor_3d(ctx, K->type, head_dim, n_head, n_kv));
        V = ggml_repeat(ctx, V, ggml_new_tensor_3d(ctx, V->type, head_dim, n_head, n_kv));
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: computing attention scores\n");
        fprintf(stderr, "[DEBUG] MHA[0]: q shape=[%lld,%lld,%lld,%lld]\n",
                (long long)q->ne[0], (long long)q->ne[1], (long long)q->ne[2], (long long)q->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: K shape=[%lld,%lld,%lld,%lld]\n",
                (long long)K->ne[0], (long long)K->ne[1], (long long)K->ne[2], (long long)K->ne[3]);
        fflush(stderr);
    }

    // Compute attention scores: Q @ K^T
    // q: [head_dim, n_head, n_tokens] -> [head_dim, n_tokens, n_head]
    // K: [head_dim, n_head, n_kv] -> [head_dim, n_kv, n_head]
    // For ggml_mul_mat: need ne[0] and ne[2] to match

    // Permute Q and K so that batch dimension (n_head) is in ne[2]
    q = ggml_permute(ctx, q, 0, 2, 1, 3);  // [head_dim, n_tokens, n_head, 1]
    K = ggml_permute(ctx, K, 0, 2, 1, 3);  // [head_dim, n_kv, n_head, 1]

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: q shape=[%lld,%lld,%lld,%lld] (after permute)\n",
                (long long)q->ne[0], (long long)q->ne[1], (long long)q->ne[2], (long long)q->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: K shape=[%lld,%lld,%lld,%lld] (after permute)\n",
                (long long)K->ne[0], (long long)K->ne[1], (long long)K->ne[2], (long long)K->ne[3]);
        fflush(stderr);
    }

    // scores = q @ K^T: [n_kv, n_tokens, n_head]
    struct ggml_tensor* scores = ggml_mul_mat(ctx, K, q);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores shape=[%lld,%lld,%lld,%lld]\n",
                (long long)scores->ne[0], (long long)scores->ne[1], (long long)scores->ne[2], (long long)scores->ne[3]);
        fflush(stderr);
    }

    // Scale
    const float scale = 1.0f / sqrtf((float)head_dim);
    scores = ggml_scale(ctx, scores, scale);

    // Permute scores for causal mask: [n_kv, n_tokens, n_head] -> [n_kv, n_head, n_tokens]
    scores = ggml_cont(ctx, ggml_permute(ctx, scores, 0, 2, 1, 3));

    // Causal mask
    scores = ggml_diag_mask_inf(ctx, scores, n_past);

    // Softmax over n_kv dimension (ne[0])
    scores = ggml_soft_max(ctx, scores);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores computed, applying to values\n");
        fflush(stderr);
    }

    // Apply attention to values: scores @ V
    // scores: [n_kv, n_head, n_tokens] (after diag_mask and softmax)
    // V: [head_dim, n_head, n_kv]
    // We need: attn_out = scores @ V -> [head_dim, n_head, n_tokens]

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores shape=[%lld,%lld,%lld,%lld] (before permute for V)\n",
                (long long)scores->ne[0], (long long)scores->ne[1], (long long)scores->ne[2], (long long)scores->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: V shape=[%lld,%lld,%lld,%lld] (before permute)\n",
                (long long)V->ne[0], (long long)V->ne[1], (long long)V->ne[2], (long long)V->ne[3]);
        fflush(stderr);
    }

    // For ggml_mul_mat(a, b) = b @ a^T:
    // - need a->ne[0] == b->ne[0], a->ne[2] == b->ne[2], a->ne[3] == b->ne[3]
    //
    // scores: [n_kv, n_head, n_tokens] -> permute -> [n_kv, n_tokens, n_head]
    // V: [head_dim, n_head, n_kv] -> permute -> [n_kv, head_dim, n_head]
    // Then mul_mat(V', scores') where ne[0]=n_kv matches, ne[2]=n_head matches

    // Permute scores: [n_kv, n_head, n_tokens] -> [n_kv, n_tokens, n_head]
    scores = ggml_cont(ctx, ggml_permute(ctx, scores, 0, 2, 1, 3));

    // Permute V: [head_dim, n_head, seq_len] -> [seq_len, head_dim, n_head]
    // V->ne = [64, 14, 31, 1] = [head_dim, n_head, seq_len, batch]
    // We need [31, 64, 14, 1] = [seq_len, head_dim, n_head, batch]
    // ggml_permute semantics: result->ne[axis_i] = input->ne[i]
    // So permute(1, 2, 0, 3) means:
    //   result->ne[1] = V->ne[0] = 64 (head_dim)
    //   result->ne[2] = V->ne[1] = 14 (n_head)
    //   result->ne[0] = V->ne[2] = 31 (seq_len)
    //   result->ne[3] = V->ne[3] = 1 (batch)
    // Result: [31, 64, 14, 1]
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: V permute params=(1,2,0,3) V2\n");
        fflush(stderr);
    }
    V = ggml_cont(ctx, ggml_permute(ctx, V, 1, 2, 0, 3));

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores shape=[%lld,%lld,%lld,%lld] (for V matmul)\n",
                (long long)scores->ne[0], (long long)scores->ne[1], (long long)scores->ne[2], (long long)scores->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: V shape=[%lld,%lld,%lld,%lld] (after permute)\n",
                (long long)V->ne[0], (long long)V->ne[1], (long long)V->ne[2], (long long)V->ne[3]);
        fflush(stderr);
    }

    // ggml_mul_mat(V, scores) = scores @ V^T
    // V: [n_kv, head_dim, n_head], V^T: [head_dim, n_kv, n_head]
    // scores: [n_kv, n_tokens, n_head]
    // Result: [head_dim, n_tokens, n_head]
    struct ggml_tensor* attn_out = ggml_mul_mat(ctx, V, scores);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out shape=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->ne[0], (long long)attn_out->ne[1], (long long)attn_out->ne[2], (long long)attn_out->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: output projection\n");
        fflush(stderr);
    }

    // Reshape: [head_dim, n_tokens, n_head] -> [head_dim, n_head, n_tokens] -> [n_embd, n_tokens]
    attn_out = ggml_cont(ctx, ggml_permute(ctx, attn_out, 0, 2, 1, 3));  // [head_dim, n_head, n_tokens]
    attn_out = ggml_reshape_2d(ctx, attn_out, n_embd, n_tokens);
    attn_out = ggml_mul_mat(ctx, layer.wo, attn_out);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: done\n");
        fflush(stderr);
    }

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
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: cur shape=[%lld, %lld]\n",
                (long long)cur->ne[0], (long long)cur->ne[1]);
        fprintf(stderr, "[DEBUG] build_layer[0]: attn_norm=%p, wq=%p, wk=%p, wv=%p, wo=%p\n",
                (void*)layer.attn_norm, (void*)layer.wq, (void*)layer.wk, (void*)layer.wv, (void*)layer.wo);
        fprintf(stderr, "[DEBUG] build_layer[0]: ffn_norm=%p, gate=%p, up=%p, down=%p\n",
                (void*)layer.ffn_norm, (void*)layer.ffn_gate, (void*)layer.ffn_up, (void*)layer.ffn_down);
        fprintf(stderr, "[DEBUG] build_layer[0]: k_cache=%p, v_cache=%p\n",
                (void*)k_cache, (void*)v_cache);
        fflush(stderr);
    }

    struct ggml_tensor* residual = cur;

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling rms_norm (attn)\n");
        fflush(stderr);
    }

    // Pre-attention RMSNorm
    cur = rms_norm(ctx, cur, layer.attn_norm, hparams.norm_eps);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling multi_head_attention\n");
        fflush(stderr);
    }

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

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: attention done, adding residual\n");
        fflush(stderr);
    }

    // Residual connection
    cur = ggml_add(ctx, cur, residual);
    residual = cur;

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling rms_norm (ffn)\n");
        fflush(stderr);
    }

    // Pre-FFN RMSNorm
    cur = rms_norm(ctx, cur, layer.ffn_norm, hparams.norm_eps);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling swiglu_ffn\n");
        fflush(stderr);
    }

    // FFN
    cur = swiglu_ffn(ctx, cur, layer);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: ffn done, adding residual\n");
        fflush(stderr);
    }

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
    fprintf(stderr, "[DEBUG] build_compute_graph: entered\n");
    fflush(stderr);

    GgmlModel* model = ctx->model;
    const ModelHParams& hparams = model->hparams;
    const ModelTensors& tensors = model->tensors;

    fprintf(stderr, "[DEBUG] build_compute_graph: n_layer=%d, n_embd=%d, n_vocab=%d\n",
            hparams.n_layer, hparams.n_embd, hparams.n_vocab);
    fflush(stderr);

    // Estimate buffer size for compute
    size_t compute_size = estimate_compute_buffer_size(hparams, ctx->kv_size, n_tokens);
    fprintf(stderr, "[DEBUG] build_compute_graph: compute_size=%zu bytes\n", compute_size);
    fflush(stderr);

    struct ggml_init_params graph_params = {
        .mem_size = compute_size,
        .mem_buffer = nullptr,
        .no_alloc = true,  // Let graph allocator handle tensor buffer allocation
    };

    // Clear positions tensors list for this graph build
    get_positions_storage().count = 0;

    struct ggml_context* ctx_graph = ggml_init(graph_params);
    if (!ctx_graph) {
        fprintf(stderr, "[DEBUG] build_compute_graph: ggml_init failed\n");
        fflush(stderr);
        return nullptr;
    }
    fprintf(stderr, "[DEBUG] build_compute_graph: ggml_init succeeded\n");
    fflush(stderr);

    // Create embedding input tensor directly (data will be set in forward_pass)
    // We don't use ggml_get_rows because the graph allocator reuses inp_tokens buffer
    struct ggml_tensor* cur = ggml_new_tensor_2d(ctx_graph, GGML_TYPE_F32, hparams.n_embd, n_tokens);
    ggml_set_name(cur, "emb_input");
    ggml_set_input(cur);  // Mark as input so allocator will allocate it
    fprintf(stderr, "[DEBUG] build_compute_graph: embedding input tensor created [%d, %d]\n",
            hparams.n_embd, n_tokens);
    fflush(stderr);

    // Process each transformer layer
    for (int i = 0; i < hparams.n_layer; ++i) {
        if (i % 4 == 0) {
            fprintf(stderr, "[DEBUG] build_compute_graph: processing layer %d/%d\n", i, hparams.n_layer);
            fflush(stderr);
        }
        cur = build_layer(
            ctx_graph, cur,
            tensors.layers[i],
            ctx->k_cache, ctx->v_cache,
            n_past, n_tokens,
            hparams, i
        );
    }
    fprintf(stderr, "[DEBUG] build_compute_graph: all layers processed\n");
    fflush(stderr);

    // Final RMSNorm
    cur = rms_norm(ctx_graph, cur, tensors.output_norm, hparams.norm_eps);
    ggml_set_name(cur, "final_norm");

    // LM head - check output tensor has data
    if (tensors.output && tensors.output->buffer) {
        std::vector<float> output_data(5);
        ggml_backend_tensor_get(tensors.output, output_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] build_compute_graph: output tensor first 5: %.6f %.6f %.6f %.6f %.6f\n",
                output_data[0], output_data[1], output_data[2], output_data[3], output_data[4]);
        fflush(stderr);
    }
    cur = ggml_mul_mat(ctx_graph, tensors.output, cur);
    fprintf(stderr, "[DEBUG] build_compute_graph: output tensor ne=[%lld, %lld], output_norm ne=[%lld]\n",
            (long long)tensors.output->ne[0], (long long)tensors.output->ne[1],
            (long long)tensors.output_norm->ne[0]);
    fflush(stderr);

    ggml_set_name(cur, "logits");
    ggml_set_output(cur);  // Mark as output so allocator will allocate it
    fprintf(stderr, "[DEBUG] build_compute_graph: logits computed (marked as output)\n");
    fflush(stderr);

    // Build graph
    struct ggml_cgraph* graph = ggml_new_graph(ctx_graph);
    ggml_build_forward_expand(graph, cur);

    fprintf(stderr, "[DEBUG] build_compute_graph: graph built, returning\n");
    fflush(stderr);

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
    fprintf(stderr, "[DEBUG] forward_pass: starting, n_tokens=%d, n_past=%d\n", n_tokens, n_past);
    fflush(stderr);

    if (!ctx || !ctx->model) {
        error = "Invalid context";
        return false;
    }

    // Check cancel flag
    if (ctx->cancel_flag.load(std::memory_order_acquire)) {
        error = "Cancelled";
        return false;
    }

    fprintf(stderr, "[DEBUG] forward_pass: building compute graph\n");
    fflush(stderr);

    // Build compute graph
    struct ggml_cgraph* graph = build_compute_graph(ctx, tokens, n_tokens, n_past);
    if (!graph) {
        error = "Failed to build compute graph";
        return false;
    }

    fprintf(stderr, "[DEBUG] forward_pass: graph built, n_nodes=%d\n", ggml_graph_n_nodes(graph));
    fflush(stderr);

    // Get backend
    ggml_backend_t backend = ctx->model->backend;

    fprintf(stderr, "[DEBUG] forward_pass: creating graph allocator\n");
    fflush(stderr);

    // Create graph allocator for the backend
    ggml_gallocr_t allocr = ggml_gallocr_new(ggml_backend_get_default_buffer_type(backend));
    if (!allocr) {
        error = "Failed to create graph allocator";
        return false;
    }

    // Reserve memory for the graph
    if (!ggml_gallocr_reserve(allocr, graph)) {
        ggml_gallocr_free(allocr);
        error = "Failed to reserve graph memory";
        return false;
    }

    // Allocate graph tensors
    if (!ggml_gallocr_alloc_graph(allocr, graph)) {
        ggml_gallocr_free(allocr);
        error = "Failed to allocate graph tensors";
        return false;
    }

    fprintf(stderr, "[DEBUG] forward_pass: graph tensors allocated\n");
    fflush(stderr);

    // Set embedding input: copy embeddings directly from tok_embd to emb_input tensor
    struct ggml_tensor* emb_input = ggml_graph_get_tensor(graph, "emb_input");
    if (emb_input && emb_input->buffer && ctx->model->tensors.tok_embd) {
        size_t n_embd = ctx->model->hparams.n_embd;
        size_t emb_byte_size = n_embd * sizeof(float);
        auto& tok_embd = ctx->model->tensors.tok_embd;

        fprintf(stderr, "[DEBUG] forward_pass: setting embedding input for %d tokens\n", n_tokens);
        fprintf(stderr, "[DEBUG] forward_pass: token[0]=%d, tok_embd nb[1]=%zu\n", tokens[0], tok_embd->nb[1]);
        fflush(stderr);

        // Allocate buffer for all token embeddings
        std::vector<float> emb_buffer(n_embd * n_tokens);

        // Copy each token's embedding
        for (int i = 0; i < n_tokens; i++) {
            int32_t token_id = tokens[i];
            size_t src_offset = static_cast<size_t>(token_id) * tok_embd->nb[1];

            // Read embedding from tok_embd
            ggml_backend_tensor_get(tok_embd, emb_buffer.data() + i * n_embd,
                                    src_offset, emb_byte_size);
        }

        // Write embeddings to emb_input
        ggml_backend_tensor_set(emb_input, emb_buffer.data(), 0, emb_byte_size * n_tokens);

        fprintf(stderr, "[DEBUG] forward_pass: embedding input set, first 5: %.6f %.6f %.6f %.6f %.6f\n",
                emb_buffer[0], emb_buffer[1], emb_buffer[2], emb_buffer[3], emb_buffer[4]);
        fflush(stderr);
    } else {
        ggml_gallocr_free(allocr);
        error = "emb_input tensor not found or no buffer";
        return false;
    }

    // Set positions tensors data after allocation
    auto& positions_storage = get_positions_storage();
    fprintf(stderr, "[DEBUG] forward_pass: setting %d positions tensors\n", positions_storage.count);
    fflush(stderr);
    for (int i = 0; i < positions_storage.count; ++i) {
        const auto& info = positions_storage.tensors[i];
        if (!info.tensor || !info.tensor->buffer) {
            ggml_gallocr_free(allocr);
            error = "Positions tensor " + std::to_string(i) + " has no buffer";
            return false;
        }
        // Create position data
        std::vector<int32_t> pos_data(info.n_tokens);
        for (int j = 0; j < info.n_tokens; ++j) {
            pos_data[j] = info.n_past + j;
        }
        ggml_backend_tensor_set(info.tensor, pos_data.data(), 0, info.n_tokens * sizeof(int32_t));
    }
    fprintf(stderr, "[DEBUG] forward_pass: positions tensors set\n");
    fflush(stderr);

    fprintf(stderr, "[DEBUG] forward_pass: starting backend compute\n");
    fflush(stderr);

    // Run computation
    enum ggml_status status = ggml_backend_graph_compute(backend, graph);

    if (status != GGML_STATUS_SUCCESS) {
        ggml_gallocr_free(allocr);
        error = "Backend compute failed with status " + std::to_string(static_cast<int>(status));
        return false;
    }

    fprintf(stderr, "[DEBUG] forward_pass: backend compute done\n");
    fflush(stderr);

    // Debug: check embeddings tensor after compute
    struct ggml_tensor* emb_tensor = ggml_graph_get_tensor(graph, "emb_input");
    if (emb_tensor && emb_tensor->buffer) {
        std::vector<float> emb_data(5);
        ggml_backend_tensor_get(emb_tensor, emb_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: emb_input first 5 values after compute: %.6f %.6f %.6f %.6f %.6f\n",
                emb_data[0], emb_data[1], emb_data[2], emb_data[3], emb_data[4]);
        fflush(stderr);
    }

    // Debug: check final_norm tensor after compute
    struct ggml_tensor* final_norm_tensor = ggml_graph_get_tensor(graph, "final_norm");
    if (final_norm_tensor && final_norm_tensor->buffer) {
        std::vector<float> norm_data(5);
        ggml_backend_tensor_get(final_norm_tensor, norm_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: final_norm first 5 values: %.6f %.6f %.6f %.6f %.6f\n",
                norm_data[0], norm_data[1], norm_data[2], norm_data[3], norm_data[4]);
        fflush(stderr);
    }

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
        ggml_gallocr_free(allocr);
        error = "Logits tensor not found";
        return false;
    }

    fprintf(stderr, "[DEBUG] forward_pass: copying logits\n");
    fprintf(stderr, "[DEBUG] forward_pass: logits_tensor ne=[%lld, %lld, %lld, %lld]\n",
            (long long)logits_tensor->ne[0], (long long)logits_tensor->ne[1],
            (long long)logits_tensor->ne[2], (long long)logits_tensor->ne[3]);
    fprintf(stderr, "[DEBUG] forward_pass: logits_tensor data=%p, buffer=%p\n",
            logits_tensor->data, (void*)logits_tensor->buffer);
    fflush(stderr);

    // Copy logits for the last token
    const ModelHParams& hparams = ctx->model->hparams;
    size_t logits_size = hparams.n_vocab * sizeof(float);

    // Get logits for last token position (logits tensor is [n_vocab, n_tokens])
    // We want the last token's logits
    size_t offset = (n_tokens - 1) * hparams.n_vocab * sizeof(float);

    // For backend compute, we need to use ggml_backend_tensor_get to read the result
    // Check if tensor has a buffer (backend tensor)
    fprintf(stderr, "[DEBUG] forward_pass: offset=%zu, logits_size=%zu, tensor_size=%zu\n",
            offset, logits_size, ggml_nbytes(logits_tensor));
    fflush(stderr);

    bool logits_copied = false;
    fprintf(stderr, "[DEBUG] forward_pass: dest logits ptr=%p\n", (void*)logits);
    fflush(stderr);
    if (logits_tensor->buffer) {
        fprintf(stderr, "[DEBUG] forward_pass: have buffer, path A\n");
        fflush(stderr);
        // Verify bounds
        bool bounds_ok = (offset + logits_size <= ggml_nbytes(logits_tensor));
        fprintf(stderr, "[DEBUG] forward_pass: bounds_ok=%d\n", bounds_ok ? 1 : 0);
        fflush(stderr);
        if (!bounds_ok) {
            ggml_gallocr_free(allocr);
            error = "Logits offset+size exceeds tensor size";
            return false;
        }
        // Synchronize backend before reading
        fprintf(stderr, "[DEBUG] forward_pass: calling ggml_backend_synchronize\n");
        fflush(stderr);
        ggml_backend_synchronize(backend);
        fprintf(stderr, "[DEBUG] forward_pass: sync done, calling tensor_get\n");
        fflush(stderr);
        ggml_backend_tensor_get(logits_tensor, logits, offset, logits_size);
        logits_copied = true;
        fprintf(stderr, "[DEBUG] forward_pass: ggml_backend_tensor_get completed\n");
        fflush(stderr);
    } else if (logits_tensor->data) {
        fprintf(stderr, "[DEBUG] forward_pass: using direct memcpy\n");
        fflush(stderr);
        const float* src = (const float*)((char*)logits_tensor->data + offset);
        memcpy(logits, src, logits_size);
        logits_copied = true;
    }

    // Free allocator AFTER reading logits (buffers are freed with allocator)
    ggml_gallocr_free(allocr);

    if (!logits_copied) {
        error = "Logits tensor has no data";
        return false;
    }

    // Debug: check first few logit values
    fprintf(stderr, "[DEBUG] forward_pass: first 5 logits: %.4f %.4f %.4f %.4f %.4f\n",
            logits[0], logits[1], logits[2], logits[3], logits[4]);
    fflush(stderr);

    // Update KV cache position
    ctx->kv_used = n_past + n_tokens;

    fprintf(stderr, "[DEBUG] forward_pass: done\n");
    fflush(stderr);

    return true;
}

}  // namespace stcpp
