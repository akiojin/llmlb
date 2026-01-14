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
#include <algorithm>
#include <cmath>
#include <cstring>

namespace stcpp {

/* RMSNorm debug counter (static, not thread_local to avoid plugin issues) */
static int g_rms_debug_counter = 0;

/* RMSNorm operation */
static struct ggml_tensor* rms_norm(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* weight,
    float eps
) {
    int call_idx = g_rms_debug_counter++;

    // Debug: print input and weight shapes for first call
    if (call_idx == 0) {
        fprintf(stderr, "[DEBUG] rms_norm[0]: input x shape=[%lld, %lld, %lld, %lld]\n",
                (long long)x->ne[0], (long long)x->ne[1], (long long)x->ne[2], (long long)x->ne[3]);
        fprintf(stderr, "[DEBUG] rms_norm[0]: weight shape=[%lld, %lld, %lld, %lld]\n",
                (long long)weight->ne[0], (long long)weight->ne[1], (long long)weight->ne[2], (long long)weight->ne[3]);
        fprintf(stderr, "[DEBUG] rms_norm[0]: weight name=%s, buffer=%p\n",
                weight->name, (void*)weight->buffer);
        fflush(stderr);
    }

    // RMSNorm(x) = x / sqrt(mean(x^2) + eps) * weight
    x = ggml_rms_norm(ctx, x, eps);

    // Name for debugging (only first call which is layer 0 attention)
    if (call_idx == 0) {
        ggml_set_name(x, "debug_rms_raw");
    }

    // IMPORTANT: Mark rms_norm output to prevent gallocr from reusing its buffer
    // before ggml_cont can copy it (fixes buffer aliasing issue)
    ggml_set_output(x);

    // Use ggml_cont to force a copy, preventing buffer aliasing issues
    // This ensures the MUL operation writes to a different buffer than the input
    x = ggml_cont(ctx, x);

    if (call_idx == 0) {
        ggml_set_name(x, "debug_rms_cont");
    }

    // Also mark ggml_cont output to prevent its buffer from being reused
    ggml_set_output(x);

    struct ggml_tensor* result = ggml_mul(ctx, x, weight);
    if (call_idx == 0) {
        ggml_set_name(result, "debug_rms_mul");
        ggml_set_output(result);
    }

    return result;
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
    static PositionsStorage storage = {{}, 0};
    return storage;
}

/* Thread-local storage for KV cache copy tensors (must be added to graph explicitly) */
struct CopyTensorsStorage {
    struct ggml_tensor* tensors[256];  // Max 128 layers * 2 (k, v)
    int count;
};

static CopyTensorsStorage& get_copy_storage() {
    static CopyTensorsStorage storage = {{}, 0};
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

    // Use GPT-NeoX style RoPE (GGML_ROPE_TYPE_NEOX = 2)
    // Qwen2.5 uses rotate_half which splits the vector in halves,
    // not interleaved pairs like the standard RoPE (mode=0)
    const int mode = 2;

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] apply_rope[0]: freq_base=%.1f, freq_scale=%.4f, n_rot=%d, mode=%d\n",
                freq_base, freq_scale, n_rot, mode);
        fflush(stderr);
    }

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


/**
 * GQA expansion helper: expand K or V from [head_dim, n_head_kv, n_kv] to [head_dim, n_head, n_kv]
 * by repeating each KV head n_rep times consecutively.
 *
 * For n_head=14, n_head_kv=2, this produces the pattern:
 * [KV0, KV0, KV0, KV0, KV0, KV0, KV0, KV1, KV1, KV1, KV1, KV1, KV1, KV1]
 *
 * ggml_repeat cannot be used because it produces a tiled pattern (0,1,0,1,...)
 * instead of the required interleaved pattern (0,0,0,...,1,1,1,...).
 */
static struct ggml_tensor* gqa_expand_kv(
    struct ggml_context* ctx,
    struct ggml_tensor* kv,       // [head_dim, n_head_kv, n_kv]
    int32_t n_head,
    int32_t n_head_kv,
    int32_t layer_idx
) {
    if (n_head_kv >= n_head) {
        return kv;  // No expansion needed (MHA case)
    }

    const int32_t n_rep = n_head / n_head_kv;
    const int64_t head_dim = kv->ne[0];
    const int64_t n_kv_len = kv->ne[2];

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] gqa_expand_kv: kv shape=[%lld,%lld,%lld], n_head=%d, n_head_kv=%d, n_rep=%d\n",
                (long long)kv->ne[0], (long long)kv->ne[1], (long long)kv->ne[2],
                n_head, n_head_kv, n_rep);
        fflush(stderr);
    }

    // Strategy: For each KV head, repeat it n_rep times using views and concat
    // This produces the correct GQA pattern: [kv0,kv0,kv0,kv0,kv0,kv0,kv0,kv1,kv1,kv1,kv1,kv1,kv1,kv1]
    //
    // For kv with shape [head_dim, n_head_kv, n_kv]:
    // - Extract each kv head: kv[:, h, :] with shape [head_dim, 1, n_kv]
    // - Use ggml_repeat to repeat this head n_rep times along dim 1: [head_dim, n_rep, n_kv]
    // - Concat all repeated heads along dim 1 to get [head_dim, n_head, n_kv]

    struct ggml_tensor* result = nullptr;

    for (int32_t kv_h = 0; kv_h < n_head_kv; kv_h++) {
        // Create a view for this KV head: kv[:, kv_h:kv_h+1, :]
        // ggml_view_3d(ctx, a, ne0, ne1, ne2, nb1, nb2, offset)
        // - ne0, ne1, ne2: shape of the view
        // - nb1, nb2: strides from original tensor
        // - offset: byte offset into original tensor's data
        struct ggml_tensor* kv_head = ggml_view_3d(
            ctx, kv,
            head_dim, 1, n_kv_len,           // shape: [head_dim, 1, n_kv]
            kv->nb[1], kv->nb[2],            // strides from original
            kv_h * kv->nb[1]                 // offset to this head
        );

        // Make contiguous for repeat
        kv_head = ggml_cont(ctx, kv_head);

        // Repeat this head n_rep times along dimension 1
        // ggml_repeat tiles the tensor to match target shape
        struct ggml_tensor* kv_head_repeated = ggml_repeat(
            ctx, kv_head,
            ggml_new_tensor_3d(ctx, kv->type, head_dim, n_rep, n_kv_len)
        );

        if (layer_idx == 0) {
            fprintf(stderr, "[DEBUG] gqa_expand_kv: kv_head[%d] repeated shape=[%lld,%lld,%lld]\n",
                    kv_h, (long long)kv_head_repeated->ne[0],
                    (long long)kv_head_repeated->ne[1], (long long)kv_head_repeated->ne[2]);
            fflush(stderr);
        }

        // Concatenate with previous result along dimension 1 (heads)
        if (result == nullptr) {
            result = kv_head_repeated;
        } else {
            result = ggml_concat(ctx, result, kv_head_repeated, 1);
        }
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] gqa_expand_kv: result shape=[%lld,%lld,%lld]\n",
                (long long)result->ne[0], (long long)result->ne[1], (long long)result->ne[2]);
        fflush(stderr);
    }

    return result;
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
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: wq shape=[%lld, %lld], wk shape=[%lld, %lld], wv shape=[%lld, %lld]\n",
                (long long)layer.wq->ne[0], (long long)layer.wq->ne[1],
                (long long)layer.wk->ne[0], (long long)layer.wk->ne[1],
                (long long)layer.wv->ne[0], (long long)layer.wv->ne[1]);
        fprintf(stderr, "[DEBUG] MHA[0]: cur shape=[%lld, %lld]\n",
                (long long)cur->ne[0], (long long)cur->ne[1]);
        fflush(stderr);
    }
    struct ggml_tensor* q = ggml_mul_mat(ctx, layer.wq, cur);
    struct ggml_tensor* k = ggml_mul_mat(ctx, layer.wk, cur);
    struct ggml_tensor* v = ggml_mul_mat(ctx, layer.wv, cur);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: q shape=[%lld, %lld], k shape=[%lld, %lld], v shape=[%lld, %lld]\n",
                (long long)q->ne[0], (long long)q->ne[1],
                (long long)k->ne[0], (long long)k->ne[1],
                (long long)v->ne[0], (long long)v->ne[1]);
        fflush(stderr);
    }

    // Add biases if present (e.g., Qwen2)
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: has_bq=%d, has_bk=%d, has_bv=%d, bq=%p, bk=%p, bv=%p\n",
                layer.has_bq, layer.has_bk, layer.has_bv,
                (void*)layer.bq, (void*)layer.bk, (void*)layer.bv);
        // Print bias values for debugging
        if (layer.has_bq && layer.bq && layer.bq->buffer) {
            std::vector<float> bq_vals(5);
            ggml_backend_tensor_get(layer.bq, bq_vals.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] MHA[0]: bq first 5: %.6f %.6f %.6f %.6f %.6f\n",
                    bq_vals[0], bq_vals[1], bq_vals[2], bq_vals[3], bq_vals[4]);
        }
        if (layer.has_bk && layer.bk && layer.bk->buffer) {
            std::vector<float> bk_vals(5);
            ggml_backend_tensor_get(layer.bk, bk_vals.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] MHA[0]: bk first 5: %.6f %.6f %.6f %.6f %.6f\n",
                    bk_vals[0], bk_vals[1], bk_vals[2], bk_vals[3], bk_vals[4]);
        }
        fflush(stderr);
    }
    if (layer.has_bq && layer.bq) {
        q = ggml_add(ctx, q, layer.bq);
    }
    if (layer.has_bk && layer.bk) {
        k = ggml_add(ctx, k, layer.bk);
    }
    if (layer.has_bv && layer.bv) {
        v = ggml_add(ctx, v, layer.bv);
    }

    // Debug: mark Q after bias for inspection
    if (layer_idx == 0) {
        ggml_set_name(q, "layer0_q_after_bias");
        ggml_set_output(q);
        // Also mark V after bias for inspection
        ggml_set_name(v, "layer0_v_after_bias");
        ggml_set_output(v);
    }

    // Reshape for multi-head attention
    // Q: [n_embd, n_tokens] -> [head_dim, n_head, n_tokens]
    q = ggml_reshape_3d(ctx, q, head_dim, n_head, n_tokens);
    k = ggml_reshape_3d(ctx, k, head_dim, n_head_kv, n_tokens);
    v = ggml_reshape_3d(ctx, v, head_dim, n_head_kv, n_tokens);

    // Debug: mark Q before RoPE for inspection
    if (layer_idx == 0) {
        ggml_set_name(q, "layer0_q_before_rope");
        ggml_set_output(q);
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

    // Debug: mark Q and K after RoPE for inspection
    if (layer_idx == 0) {
        ggml_set_name(q, "layer0_q_after_rope");
        ggml_set_output(q);
        ggml_set_name(k, "layer0_k_after_rope");
        ggml_set_output(k);
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

    // Copy current K, V to cache
    // IMPORTANT: These copy operations must be added to the graph explicitly
    // since the subsequent K and V views are created from k_cache/v_cache directly
    struct ggml_tensor* k_cpy = ggml_cpy(ctx, k, k_cache_layer);
    struct ggml_tensor* v_cpy = ggml_cpy(ctx, v, v_cache_layer);

    // Track copy tensors so they can be added to the graph
    auto& copy_storage = get_copy_storage();
    if (copy_storage.count + 2 <= 256) {
        copy_storage.tensors[copy_storage.count++] = k_cpy;
        copy_storage.tensors[copy_storage.count++] = v_cpy;
    }

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
    // Mark K as input so gallocr doesn't allocate new buffer
    ggml_set_input(K);

    struct ggml_tensor* V = ggml_view_3d(
        ctx, v_cache,
        head_dim, n_head_kv, n_kv,
        v_cache->nb[1], v_cache->nb[2],
        layer_idx * v_cache->nb[3]
    );
    // Mark V as input so gallocr doesn't allocate new buffer
    ggml_set_input(V);

    // For prefill (n_past == 0): use current k, v directly
    // For decode (n_past > 0): concatenate past K, V from cache with current k, v
    if (n_past == 0) {
        // Prefill: K contains only current tokens, use k directly
        K = k;
        V = v;
    } else {
        // Decode: need to concatenate past K, V from cache with current k, v
        // Create views into the cache for past tokens only
        struct ggml_tensor* K_past = ggml_view_3d(
            ctx, k_cache,
            head_dim, n_head_kv, n_past,
            k_cache->nb[1], k_cache->nb[2],
            layer_idx * k_cache->nb[3]
        );
        ggml_set_input(K_past);
        ggml_set_name(K_past, "K_past");

        struct ggml_tensor* V_past = ggml_view_3d(
            ctx, v_cache,
            head_dim, n_head_kv, n_past,
            v_cache->nb[1], v_cache->nb[2],
            layer_idx * v_cache->nb[3]
        );
        ggml_set_input(V_past);
        ggml_set_name(V_past, "V_past");

        // Cast current k, v to F16 to match cache type
        struct ggml_tensor* k_f16 = ggml_cast(ctx, k, GGML_TYPE_F16);
        struct ggml_tensor* v_f16 = ggml_cast(ctx, v, GGML_TYPE_F16);

        // Concatenate past and current: dim 2 (n_tokens dimension)
        // ggml_concat concatenates on the third dimension (ne[2])
        K = ggml_concat(ctx, K_past, k_f16, 2);  // [head_dim, n_head_kv, n_past + n_tokens]
        V = ggml_concat(ctx, V_past, v_f16, 2);  // [head_dim, n_head_kv, n_past + n_tokens]

        // Cast back to F32 for attention computation
        K = ggml_cast(ctx, K, GGML_TYPE_F32);
        V = ggml_cast(ctx, V, GGML_TYPE_F32);

        if (layer_idx == 0) {
            fprintf(stderr, "[DEBUG] MHA[0]: decode mode, K_past shape=[%lld,%lld,%lld], K shape=[%lld,%lld,%lld]\n",
                    (long long)K_past->ne[0], (long long)K_past->ne[1], (long long)K_past->ne[2],
                    (long long)K->ne[0], (long long)K->ne[1], (long long)K->ne[2]);
            fflush(stderr);
        }
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: handling GQA (n_head=%d, n_head_kv=%d)\n", n_head, n_head_kv);
        fprintf(stderr, "[DEBUG] MHA[0]: V (before repeat) view_src=%p, buffer=%p, n_past=%d\n",
                (void*)V->view_src, (void*)V->buffer, n_past);
        fprintf(stderr, "[DEBUG] MHA[0]: k_cache->buffer=%p, v_cache->buffer=%p\n",
                (void*)k_cache->buffer, (void*)v_cache->buffer);
        // Mark V before GQA to see its values
        ggml_set_name(V, "layer0_v_before_gqa");
        ggml_set_output(V);
        fflush(stderr);
    }

    // Handle GQA: expand K, V heads using proper interleaved pattern
    // ggml_repeat produces tiled pattern (0,1,0,1,...) which is WRONG for GQA
    // GQA needs interleaved pattern (0,0,0,...,1,1,1,...) where each KV head
    // is repeated n_rep times consecutively
    K = gqa_expand_kv(ctx, K, n_head, n_head_kv, layer_idx);
    V = gqa_expand_kv(ctx, V, n_head, n_head_kv, layer_idx);

    // Debug: mark V after GQA repeat
    if (layer_idx == 0) {
        ggml_set_name(V, "layer0_v_after_gqa");
        ggml_set_output(V);
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

    // Debug: mark scores after scale
    if (layer_idx == 0) {
        ggml_set_name(scores, "layer0_scores_scaled");
        ggml_set_output(scores);
    }

    // Causal mask: scores must have shape [n_kv, n_tokens, n_head]
    // ggml_diag_mask_inf masks where key_pos (ne[0]) > n_past + query_pos (ne[1])
    // No permute needed - scores already has the correct shape!
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: causal mask: n_past=%d, n_tokens=%d, n_kv=%d\n",
                n_past, n_tokens, n_past + n_tokens);
        fprintf(stderr, "[DEBUG] MHA[0]: scores shape before mask=[%lld,%lld,%lld,%lld]\n",
                (long long)scores->ne[0], (long long)scores->ne[1], (long long)scores->ne[2], (long long)scores->ne[3]);
        fflush(stderr);
    }

    // Debug: mark scores before mask
    if (layer_idx == 0) {
        ggml_set_name(scores, "layer0_scores_before_mask");
        ggml_set_output(scores);
    }

    scores = ggml_diag_mask_inf(ctx, scores, n_past);

    // Softmax over n_kv dimension (ne[0])
    scores = ggml_soft_max(ctx, scores);

    // Debug: mark scores after softmax
    if (layer_idx == 0) {
        ggml_set_name(scores, "layer0_scores_softmax");
        ggml_set_output(scores);
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores computed, applying to values\n");
        fflush(stderr);
    }

    // Apply attention to values: scores @ V
    // scores: [n_kv, n_tokens, n_head] (after diag_mask and softmax)
    // V: [head_dim, n_head, n_kv]
    // We need: attn_out = scores @ V -> [head_dim, n_tokens, n_head]

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: scores shape=[%lld,%lld,%lld,%lld] (for V matmul)\n",
                (long long)scores->ne[0], (long long)scores->ne[1], (long long)scores->ne[2], (long long)scores->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: V shape=[%lld,%lld,%lld,%lld] (before permute)\n",
                (long long)V->ne[0], (long long)V->ne[1], (long long)V->ne[2], (long long)V->ne[3]);
        fflush(stderr);
    }

    // For ggml_mul_mat(a, b) = b @ a^T:
    // - need a->ne[0] == b->ne[0], a->ne[2] == b->ne[2], a->ne[3] == b->ne[3]
    //
    // scores: [n_kv, n_tokens, n_head] - already correct!
    // V: [head_dim, n_head, n_kv] -> permute -> [n_kv, head_dim, n_head]
    // Then mul_mat(V', scores) where ne[0]=n_kv matches, ne[2]=n_head matches

    // Permute V: [head_dim, n_head, seq_len] -> [seq_len, head_dim, n_head]
    // V->ne = [64, 14, 31, 1] = [head_dim, n_head, seq_len, batch]
    // We need [31, 64, 14, 1] = [seq_len, head_dim, n_head, batch]
    // ggml_permute semantics: result->ne[axis[i]] = input->ne[i]
    // So permute(1, 2, 0, 3) means:
    //   result->ne[1] = V->ne[0] = 64 (head_dim)
    //   result->ne[2] = V->ne[1] = 14 (n_head)
    //   result->ne[0] = V->ne[2] = 31 (seq_len)
    //   result->ne[3] = V->ne[3] = 1 (batch)
    // Result: [31, 64, 14, 1]
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: V permute params=(1,2,0,3)\n");
        fflush(stderr);
    }
    V = ggml_cont(ctx, ggml_permute(ctx, V, 1, 2, 0, 3));

    // Debug: mark V after permute for matmul
    if (layer_idx == 0) {
        ggml_set_name(V, "layer0_v_for_matmul");
        ggml_set_output(V);
    }

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

    // Debug: mark attn_out raw (before permute)
    if (layer_idx == 0) {
        ggml_set_name(attn_out, "layer0_attn_out_raw");
        ggml_set_output(attn_out);
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out_raw shape=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->ne[0], (long long)attn_out->ne[1], (long long)attn_out->ne[2], (long long)attn_out->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out_raw strides=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->nb[0], (long long)attn_out->nb[1], (long long)attn_out->nb[2], (long long)attn_out->nb[3]);
        fflush(stderr);
    }

    // attn_out has shape [head_dim, n_tokens, n_head] = [64, 20, 14]
    // For o_proj, we need shape [n_embd, n_tokens] = [896, 20] where each token's data is:
    // [head0_dim0..63, head1_dim0..63, ..., head13_dim0..63]
    //
    // The current layout [head_dim, n_tokens, n_head] in column-major means:
    // - Position 0-63: dim 0-63, token 0, head 0
    // - Position 64-127: dim 0-63, token 1, head 0 (WRONG - should be head 1, token 0)
    //
    // We need layout [head_dim, n_head, n_tokens] so that:
    // - Position 0-63: dim 0-63, head 0, token 0
    // - Position 64-127: dim 0-63, head 1, token 0 (CORRECT)
    //
    // Permute from [head_dim, n_tokens, n_head] to [head_dim, n_head, n_tokens]
    // permute(0, 2, 1, 3) maps: result[0]=in[0], result[2]=in[1], result[1]=in[2]

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out before permute shape=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->ne[0], (long long)attn_out->ne[1], (long long)attn_out->ne[2], (long long)attn_out->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out before permute strides=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->nb[0], (long long)attn_out->nb[1], (long long)attn_out->nb[2], (long long)attn_out->nb[3]);
        fflush(stderr);
    }

    // Permute to [head_dim, n_head, n_tokens] and make contiguous
    attn_out = ggml_cont(ctx, ggml_permute(ctx, attn_out, 0, 2, 1, 3));

    if (layer_idx == 0) {
        ggml_set_name(attn_out, "layer0_attn_after_permute");
        ggml_set_output(attn_out);
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out after permute shape=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->ne[0], (long long)attn_out->ne[1], (long long)attn_out->ne[2], (long long)attn_out->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out after permute strides=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->nb[0], (long long)attn_out->nb[1], (long long)attn_out->nb[2], (long long)attn_out->nb[3]);
        fflush(stderr);
    }

    // Reshape to [n_embd, n_tokens]
    attn_out = ggml_reshape_2d(ctx, attn_out, n_embd, n_tokens);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out after reshape shape=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->ne[0], (long long)attn_out->ne[1], (long long)attn_out->ne[2], (long long)attn_out->ne[3]);
        fprintf(stderr, "[DEBUG] MHA[0]: attn_out after reshape strides=[%lld,%lld,%lld,%lld]\n",
                (long long)attn_out->nb[0], (long long)attn_out->nb[1], (long long)attn_out->nb[2], (long long)attn_out->nb[3]);
        fflush(stderr);
    }

    // Debug: mark attention output before o_proj
    if (layer_idx == 0) {
        ggml_set_name(attn_out, "layer0_before_oproj");
        ggml_set_output(attn_out);
    }
    // Also mark as output for all layers to prevent buffer reuse
    ggml_set_output(attn_out);

    attn_out = ggml_mul_mat(ctx, layer.wo, attn_out);

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] MHA[0]: done\n");
        ggml_set_name(attn_out, "layer0_mha_output");
        ggml_set_output(attn_out);
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

    // IMPORTANT: Mark input as output to prevent ggml_gallocr from reusing
    // the input buffer for intermediate/output tensors. Without this,
    // the computation may produce incorrect results due to buffer aliasing.
    ggml_set_output(cur);

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

    // Save input tensor for residual connection
    // IMPORTANT: Use ggml_scale(x, 1.0f) instead of direct reference to force the
    // input tensor into the compute graph. Direct reference to input tensors may not
    // work correctly with ggml_add because input tensors are set externally and
    // ggml_add may not properly read them during computation.
    struct ggml_tensor* residual = ggml_scale(ctx, cur, 1.0f);  // Identity: residual = cur * 1.0
    ggml_set_output(residual);  // Preserve buffer

    if (layer_idx == 0) {
        ggml_set_name(residual, "layer0_residual_scaled");
        fprintf(stderr, "[DEBUG] build_layer[0]: input tensor=%p (name=%s)\n", (void*)cur, cur->name);
        fprintf(stderr, "[DEBUG] build_layer[0]: residual tensor=%p (scaled copy)\n", (void*)residual);
        fprintf(stderr, "[DEBUG] build_layer[0]: calling rms_norm (attn)\n");
        fflush(stderr);
    }

    // Pre-attention RMSNorm
    cur = rms_norm(ctx, cur, layer.attn_norm, hparams.norm_eps);
    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_attn_norm");
    }

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

    // Mark attention output to prevent buffer reuse (applies to ALL layers)
    ggml_set_output(cur);

    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_attn_raw");  // attention output before residual
        fprintf(stderr, "[DEBUG] build_layer[0]: attention done, adding residual\n");
        fprintf(stderr, "[DEBUG] build_layer[0]: cur tensor=%p shape=[%lld,%lld,%lld,%lld]\n",
                (void*)cur, (long long)cur->ne[0], (long long)cur->ne[1], (long long)cur->ne[2], (long long)cur->ne[3]);
        fprintf(stderr, "[DEBUG] build_layer[0]: residual tensor=%p shape=[%lld,%lld,%lld,%lld]\n",
                (void*)residual, (long long)residual->ne[0], (long long)residual->ne[1],
                (long long)residual->ne[2], (long long)residual->ne[3]);
        fflush(stderr);
    }

    // Residual connection #1: attention_output + input
    cur = ggml_add(ctx, cur, residual);
    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: ggml_add result tensor=%p shape=[%lld,%lld,%lld,%lld]\n",
                (void*)cur, (long long)cur->ne[0], (long long)cur->ne[1], (long long)cur->ne[2], (long long)cur->ne[3]);
        fprintf(stderr, "[DEBUG] build_layer[0]: ggml_add->src[0]=%p, src[1]=%p\n",
                (void*)cur->src[0], (void*)cur->src[1]);
        fflush(stderr);
    }
    // Mark attention+residual output to prevent buffer reuse (applies to ALL layers)
    ggml_set_output(cur);

    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_attn_out");
    }

    // Save attention output for second residual connection
    // Use ggml_scale(x, 1.0f) to create a copy (identity operation) like first residual
    residual = ggml_scale(ctx, cur, 1.0f);
    ggml_set_output(residual);  // Mark as output to prevent buffer reuse
    if (layer_idx == 0) {
        ggml_set_name(residual, "layer0_residual2");
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling rms_norm (ffn)\n");
        fflush(stderr);
    }

    // Pre-FFN RMSNorm
    cur = rms_norm(ctx, cur, layer.ffn_norm, hparams.norm_eps);
    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_ffn_norm");
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: calling swiglu_ffn\n");
        fflush(stderr);
    }

    // FFN
    cur = swiglu_ffn(ctx, cur, layer);

    // Mark FFN output to prevent buffer reuse by ggml_gallocr
    // Without this, the add operation's output buffer may alias ffn_out's buffer,
    // causing the addition to fail (output = src[0] instead of src[0] + src[1])
    ggml_set_output(cur);

    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_ffn_out");
    }

    if (layer_idx == 0) {
        fprintf(stderr, "[DEBUG] build_layer[0]: ffn done, adding residual\n");
        fprintf(stderr, "[DEBUG] build_layer[0]: cur (ffn_out)=%p, residual=%p\n", (void*)cur, (void*)residual);
        fprintf(stderr, "[DEBUG] build_layer[0]: cur->ne=[%lld,%lld], residual->ne=[%lld,%lld]\n",
                (long long)cur->ne[0], (long long)cur->ne[1],
                (long long)residual->ne[0], (long long)residual->ne[1]);
        fflush(stderr);
    }

    // Residual connection #2: ffn_output + attention_output
    cur = ggml_add(ctx, cur, residual);
    if (layer_idx == 0) {
        ggml_set_name(cur, "layer0_output");
        fprintf(stderr, "[DEBUG] build_layer[0]: after ggml_add, cur=%p, src[0]=%p, src[1]=%p\n",
                (void*)cur, (void*)cur->src[0], (void*)cur->src[1]);
        fflush(stderr);
    }
    // Debug: mark last layer output for debugging
    if (layer_idx == 23) {
        ggml_set_name(cur, "layer23_output");
        ggml_set_output(cur);
    }
    // Debug: mark middle layer output
    if (layer_idx == 12) {
        ggml_set_name(cur, "layer12_output");
        ggml_set_output(cur);
    }

    return cur;
}

/* Build full compute graph for forward pass */
struct ggml_cgraph* build_compute_graph(
    GgmlContext* ctx,
    const int32_t* tokens,
    int32_t n_tokens,
    int32_t n_past
) {
    // Reset RMS debug counter for each new graph
    g_rms_debug_counter = 0;

    fprintf(stderr, "[DEBUG] build_compute_graph: entered\n");
    fflush(stderr);

    GgmlModel* model = ctx->model;
    const ModelHParams& hparams = model->hparams;
    const ModelTensors& tensors = model->tensors;

    fprintf(stderr, "[DEBUG] build_compute_graph: n_layer=%d, n_embd=%d, n_vocab=%d, norm_eps=%e\n",
            hparams.n_layer, hparams.n_embd, hparams.n_vocab, hparams.norm_eps);
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

    // Clear copy tensors list for this graph build
    get_copy_storage().count = 0;

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
    // IMPORTANT: Also mark as output to prevent gallocr from reusing this buffer
    // before the values are consumed by the first layer. Without this, the graph
    // allocator may reuse the buffer for intermediate computations during graph_compute,
    // corrupting the embedding values before they can be processed.
    ggml_set_output(cur);
    fprintf(stderr, "[DEBUG] build_compute_graph: embedding input tensor created [%d, %d] (marked as input+output)\n",
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

    // Debug: mark last layer output (before final_norm)
    ggml_set_name(cur, "last_layer_output");
    ggml_set_output(cur);

    // Final RMSNorm
    cur = rms_norm(ctx_graph, cur, tensors.output_norm, hparams.norm_eps);
    ggml_set_name(cur, "final_norm");
    ggml_set_output(cur);  // Mark so we can read it after compute

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

    // CRITICAL: Add KV cache copy operations to the graph explicitly
    // These copies are NOT in the dependency chain of cur (logits) because
    // K and V are views of k_cache/v_cache, not of k_cpy/v_cpy.
    // Without this, the copies won't be executed and KV cache won't be populated.
    auto& copy_storage = get_copy_storage();
    fprintf(stderr, "[DEBUG] build_compute_graph: adding %d KV cache copy operations to graph\n",
            copy_storage.count);
    fflush(stderr);
    for (int i = 0; i < copy_storage.count; ++i) {
        ggml_build_forward_expand(graph, copy_storage.tensors[i]);
    }

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

    // Debug: Check emb_input BEFORE compute
    {
        struct ggml_tensor* emb_pre = ggml_graph_get_tensor(graph, "emb_input");
        if (emb_pre && emb_pre->buffer) {
            std::vector<float> data(5);
            ggml_backend_tensor_get(emb_pre, data.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] BEFORE compute: emb_input data=%p, first 5: %.6f %.6f %.6f %.6f %.6f\n",
                    emb_pre->data, data[0], data[1], data[2], data[3], data[4]);
        }
        fflush(stderr);
    }

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

    // Debug: check KV cache contents after compute
    if (ctx->k_cache && ctx->k_cache->buffer && n_tokens > 0) {
        // Read first 10 values from k_cache (layer 0, position 0)
        std::vector<ggml_fp16_t> k_data(10);
        ggml_backend_tensor_get(ctx->k_cache, k_data.data(), 0, 10 * sizeof(ggml_fp16_t));
        fprintf(stderr, "[DEBUG] forward_pass: k_cache after compute (layer0, pos0-9): ");
        for (int i = 0; i < 10; ++i) {
            fprintf(stderr, "%.6f ", ggml_fp16_to_fp32(k_data[i]));
        }
        fprintf(stderr, "\n");
        fflush(stderr);
    }
    if (ctx->v_cache && ctx->v_cache->buffer && n_tokens > 0) {
        // Read first 10 values from v_cache (layer 0, position 0)
        std::vector<ggml_fp16_t> v_data(10);
        ggml_backend_tensor_get(ctx->v_cache, v_data.data(), 0, 10 * sizeof(ggml_fp16_t));
        fprintf(stderr, "[DEBUG] forward_pass: v_cache after compute (layer0, pos0-9): ");
        for (int i = 0; i < 10; ++i) {
            fprintf(stderr, "%.6f ", ggml_fp16_to_fp32(v_data[i]));
        }
        fprintf(stderr, "\n");
        fflush(stderr);
    }

    // Debug: check embeddings tensor after compute
    struct ggml_tensor* emb_tensor = ggml_graph_get_tensor(graph, "emb_input");
    if (emb_tensor && emb_tensor->buffer) {
        std::vector<float> emb_data(5);
        ggml_backend_tensor_get(emb_tensor, emb_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: emb_input tensor=%p, data=%p, buffer=%p\n",
                (void*)emb_tensor, emb_tensor->data, (void*)emb_tensor->buffer);
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

    // Debug: check RMS norm intermediate tensors
    struct ggml_tensor* debug_rms_raw = ggml_graph_get_tensor(graph, "debug_rms_raw");
    struct ggml_tensor* debug_rms_cont = ggml_graph_get_tensor(graph, "debug_rms_cont");
    struct ggml_tensor* debug_rms_mul = ggml_graph_get_tensor(graph, "debug_rms_mul");
    struct ggml_tensor* layer0_attn_norm = ggml_graph_get_tensor(graph, "layer0_attn_norm");

    fprintf(stderr, "[DEBUG] forward_pass: debug_rms_raw=%p, debug_rms_cont=%p, debug_rms_mul=%p\n",
            (void*)debug_rms_raw, (void*)debug_rms_cont, (void*)debug_rms_mul);

    // Print RMS norm raw output (before ggml_cont, before weight multiplication)
    // Expected Python values: 0.275, -0.930, 0.420, 0.022, -0.619
    if (debug_rms_raw && debug_rms_raw->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(debug_rms_raw, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: debug_rms_raw (ggml_rms_norm output, BEFORE cont/weight) first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    if (debug_rms_cont && debug_rms_cont->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(debug_rms_cont, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: debug_rms_cont (after ggml_cont) first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    if (debug_rms_mul && debug_rms_mul->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(debug_rms_mul, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: debug_rms_mul (after weight mul) first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    // Check backend used for computation
    fprintf(stderr, "[DEBUG] forward_pass: compute backend=%s\n", ggml_backend_name(backend));
    fflush(stderr);

    if (layer0_attn_norm && layer0_attn_norm->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_attn_norm, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_norm (after weight) first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    // Check if the model weight tensor still has correct data after compute
    if (ctx->model->tensors.layers.size() > 0) {
        auto& layer0_weight = ctx->model->tensors.layers[0].attn_norm;
        if (layer0_weight && layer0_weight->buffer) {
            std::vector<float> weight_data(10);
            ggml_backend_tensor_get(layer0_weight, weight_data.data(), 0, 10 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: model layer0 attn_norm weight first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                    weight_data[0], weight_data[1], weight_data[2], weight_data[3], weight_data[4],
                    weight_data[5], weight_data[6], weight_data[7], weight_data[8], weight_data[9]);
            fprintf(stderr, "[DEBUG] forward_pass: layer0_weight tensor=%p, data=%p, buffer=%p\n",
                    (void*)layer0_weight, layer0_weight->data, (void*)layer0_weight->buffer);
            fflush(stderr);
        }
    }

    // Check Q after bias
    struct ggml_tensor* layer0_q_after_bias = ggml_graph_get_tensor(graph, "layer0_q_after_bias");
    if (layer0_q_after_bias && layer0_q_after_bias->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_q_after_bias, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_q_after_bias first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    }

    // Check V after bias
    struct ggml_tensor* layer0_v_after_bias = ggml_graph_get_tensor(graph, "layer0_v_after_bias");
    if (layer0_v_after_bias && layer0_v_after_bias->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_v_after_bias, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_after_bias shape=[%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                (long long)layer0_v_after_bias->ne[0], (long long)layer0_v_after_bias->ne[1],
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_after_bias NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Check Q before RoPE
    struct ggml_tensor* layer0_q_before_rope = ggml_graph_get_tensor(graph, "layer0_q_before_rope");
    if (layer0_q_before_rope && layer0_q_before_rope->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_q_before_rope, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_q_before_rope first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    // Check Q after RoPE
    struct ggml_tensor* layer0_q_after_rope = ggml_graph_get_tensor(graph, "layer0_q_after_rope");
    if (layer0_q_after_rope && layer0_q_after_rope->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_q_after_rope, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_q_after_rope first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_q_after_rope NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Check K after RoPE
    struct ggml_tensor* layer0_k_after_rope = ggml_graph_get_tensor(graph, "layer0_k_after_rope");
    if (layer0_k_after_rope && layer0_k_after_rope->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_k_after_rope, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_k_after_rope first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    // Check scores after scale
    struct ggml_tensor* layer0_scores_scaled = ggml_graph_get_tensor(graph, "layer0_scores_scaled");
    if (layer0_scores_scaled && layer0_scores_scaled->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_scores_scaled, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_scores_scaled first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    }

    // Check scores before mask
    struct ggml_tensor* layer0_scores_before_mask = ggml_graph_get_tensor(graph, "layer0_scores_before_mask");
    if (layer0_scores_before_mask && layer0_scores_before_mask->buffer) {
        int64_t n_kv = layer0_scores_before_mask->ne[0];
        int64_t n_tokens = layer0_scores_before_mask->ne[1];
        fprintf(stderr, "[DEBUG] forward_pass: layer0_scores_before_mask shape=[%lld,%lld,%lld,%lld] (n_kv=%lld, n_tokens=%lld)\n",
                (long long)layer0_scores_before_mask->ne[0], (long long)layer0_scores_before_mask->ne[1],
                (long long)layer0_scores_before_mask->ne[2], (long long)layer0_scores_before_mask->ne[3],
                (long long)n_kv, (long long)n_tokens);
        // For prefill with n_tokens > 1, check last token's scores (last row)
        // For generation with n_tokens = 1, check the only row
        int64_t last_token_offset = (n_tokens - 1) * n_kv;
        std::vector<float> last_row(std::min(n_kv, (int64_t)10));
        ggml_backend_tensor_get(layer0_scores_before_mask, last_row.data(), last_token_offset * sizeof(float), last_row.size() * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: scores_before_mask last_token first %zu: ", last_row.size());
        for (size_t i = 0; i < last_row.size(); i++) fprintf(stderr, "%.4f ", last_row[i]);
        fprintf(stderr, "\n");
        fflush(stderr);
    }

    // Check scores after softmax
    struct ggml_tensor* layer0_scores_softmax = ggml_graph_get_tensor(graph, "layer0_scores_softmax");
    if (layer0_scores_softmax && layer0_scores_softmax->buffer) {
        int64_t n_kv = layer0_scores_softmax->ne[0];
        int64_t n_tokens = layer0_scores_softmax->ne[1];
        fprintf(stderr, "[DEBUG] forward_pass: layer0_scores_softmax shape=[%lld,%lld,%lld,%lld] (n_kv=%lld, n_tokens=%lld)\n",
                (long long)layer0_scores_softmax->ne[0], (long long)layer0_scores_softmax->ne[1],
                (long long)layer0_scores_softmax->ne[2], (long long)layer0_scores_softmax->ne[3],
                (long long)n_kv, (long long)n_tokens);
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_scores_softmax, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_scores_softmax first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        // Shape is now [n_kv, n_tokens, n_head] after fix
        // Check softmax sum for the first token, head 0
        float softmax_sum = 0.0f;
        std::vector<float> softmax_col(n_kv);
        ggml_backend_tensor_get(layer0_scores_softmax, softmax_col.data(), 0, n_kv * sizeof(float));
        for (int i = 0; i < n_kv; i++) softmax_sum += softmax_col[i];
        fprintf(stderr, "[DEBUG] forward_pass: softmax sum for token0, head0 = %.6f (expected: 1.0)\n", softmax_sum);
        fflush(stderr);
    }

    struct ggml_tensor* layer0_residual_scaled = ggml_graph_get_tensor(graph, "layer0_residual_scaled");
    if (layer0_residual_scaled && layer0_residual_scaled->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_residual_scaled, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual_scaled tensor=%p, data=%p, buffer=%p\n",
                (void*)layer0_residual_scaled, layer0_residual_scaled->data, (void*)layer0_residual_scaled->buffer);
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual_scaled (scaled input) first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual_scaled NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    struct ggml_tensor* layer0_mha_output = ggml_graph_get_tensor(graph, "layer0_mha_output");
    if (layer0_mha_output && layer0_mha_output->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_mha_output, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_mha_output first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_mha_output NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Debug: V before GQA repeat [head_dim, n_head_kv, n_kv]
    struct ggml_tensor* layer0_v_before_gqa = ggml_graph_get_tensor(graph, "layer0_v_before_gqa");
    if (layer0_v_before_gqa) {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_before_gqa tensor=%p, buffer=%p, view_src=%p, data=%p\n",
                (void*)layer0_v_before_gqa, (void*)layer0_v_before_gqa->buffer,
                (void*)layer0_v_before_gqa->view_src, layer0_v_before_gqa->data);
        if (layer0_v_before_gqa->buffer) {
            std::vector<float> data(10);
            ggml_backend_tensor_get(layer0_v_before_gqa, data.data(), 0, 10 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: layer0_v_before_gqa shape=[%lld,%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                    (long long)layer0_v_before_gqa->ne[0], (long long)layer0_v_before_gqa->ne[1], (long long)layer0_v_before_gqa->ne[2],
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        } else {
            fprintf(stderr, "[DEBUG] forward_pass: layer0_v_before_gqa has no buffer\n");
        }
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_before_gqa NOT FOUND\n");
        fflush(stderr);
    }

    // Debug: V after GQA repeat [head_dim, n_head, n_kv]
    struct ggml_tensor* layer0_v_after_gqa = ggml_graph_get_tensor(graph, "layer0_v_after_gqa");
    if (layer0_v_after_gqa && layer0_v_after_gqa->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_v_after_gqa, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_after_gqa shape=[%lld,%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                (long long)layer0_v_after_gqa->ne[0], (long long)layer0_v_after_gqa->ne[1], (long long)layer0_v_after_gqa->ne[2],
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);

        // GQA verification: positions 64-73 (head 1) should equal positions 0-9 (head 0)
        // Because with n_head_kv=2, n_rep=7, heads 0-6 should all be copies of KV head 0
        std::vector<float> head0_data(10);
        std::vector<float> head1_data(10);
        ggml_backend_tensor_get(layer0_v_after_gqa, head0_data.data(), 0, 10 * sizeof(float));
        ggml_backend_tensor_get(layer0_v_after_gqa, head1_data.data(), 64 * sizeof(float), 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: GQA verify head0[0:10]: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                head0_data[0], head0_data[1], head0_data[2], head0_data[3], head0_data[4],
                head0_data[5], head0_data[6], head0_data[7], head0_data[8], head0_data[9]);
        fprintf(stderr, "[DEBUG] forward_pass: GQA verify head1[0:10]: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                head1_data[0], head1_data[1], head1_data[2], head1_data[3], head1_data[4],
                head1_data[5], head1_data[6], head1_data[7], head1_data[8], head1_data[9]);
        bool gqa_match = true;
        for (int i = 0; i < 10; i++) {
            if (std::abs(head0_data[i] - head1_data[i]) > 1e-5f) {
                gqa_match = false;
                break;
            }
        }
        fprintf(stderr, "[DEBUG] forward_pass: GQA head0==head1? %s\n", gqa_match ? "YES" : "NO");

        // Also check head 7 (first head of KV head 1, at position 7*64=448)
        std::vector<float> head7_data(10);
        ggml_backend_tensor_get(layer0_v_after_gqa, head7_data.data(), 448 * sizeof(float), 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: GQA verify head7[0:10]: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                head7_data[0], head7_data[1], head7_data[2], head7_data[3], head7_data[4],
                head7_data[5], head7_data[6], head7_data[7], head7_data[8], head7_data[9]);
        bool head0_ne_head7 = false;
        for (int i = 0; i < 10; i++) {
            if (std::abs(head0_data[i] - head7_data[i]) > 1e-5f) {
                head0_ne_head7 = true;
                break;
            }
        }
        fprintf(stderr, "[DEBUG] forward_pass: GQA head0!=head7? %s (expected: YES, different KV heads)\n",
                head0_ne_head7 ? "YES" : "NO");

        // Check position 36 (dim 36, head 0) vs position 100 (dim 36, head 1)
        // They should be equal since head 1 is a copy of KV head 0
        float v_pos36, v_pos100;
        ggml_backend_tensor_get(layer0_v_after_gqa, &v_pos36, 36 * sizeof(float), sizeof(float));
        ggml_backend_tensor_get(layer0_v_after_gqa, &v_pos100, 100 * sizeof(float), sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: v_after_gqa[36] (dim36,h0,t0)=%.8f, v_after_gqa[100] (dim36,h1,t0)=%.8f, match=%s\n",
                v_pos36, v_pos100, (std::abs(v_pos36 - v_pos100) < 1e-5f) ? "YES" : "NO");

        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_after_gqa NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Debug: V after permute for matmul [n_kv, head_dim, n_head]
    struct ggml_tensor* layer0_v_for_matmul = ggml_graph_get_tensor(graph, "layer0_v_for_matmul");
    if (layer0_v_for_matmul && layer0_v_for_matmul->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_v_for_matmul, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_for_matmul shape=[%lld,%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                (long long)layer0_v_for_matmul->ne[0], (long long)layer0_v_for_matmul->ne[1], (long long)layer0_v_for_matmul->ne[2],
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_v_for_matmul NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Debug: attn_out raw (before permute) [head_dim, n_tokens, n_head]
    struct ggml_tensor* layer0_attn_out_raw = ggml_graph_get_tensor(graph, "layer0_attn_out_raw");
    if (layer0_attn_out_raw && layer0_attn_out_raw->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_attn_out_raw, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_out_raw shape=[%lld,%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                (long long)layer0_attn_out_raw->ne[0], (long long)layer0_attn_out_raw->ne[1], (long long)layer0_attn_out_raw->ne[2],
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        // Check element at offset 100
        std::vector<float> data100(10);
        ggml_backend_tensor_get(layer0_attn_out_raw, data100.data(), 100 * sizeof(float), 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_out_raw elements [100:110]: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data100[0], data100[1], data100[2], data100[3], data100[4], data100[5], data100[6], data100[7], data100[8], data100[9]);

        // Shape [head_dim, n_tokens, n_head] in column-major:
        // For dim 36, token 0, head h → position 36 + 0*head_dim + h*head_dim*n_tokens
        int64_t head_dim = layer0_attn_out_raw->ne[0];
        int64_t n_tokens_raw = layer0_attn_out_raw->ne[1];
        int64_t head0_pos36 = 36;
        int64_t head1_pos36 = 36 + 1 * head_dim * n_tokens_raw;
        int64_t total_elements = ggml_nelements(layer0_attn_out_raw);
        fprintf(stderr, "[DEBUG] forward_pass: attn_out_raw total_elements=%lld, head0_pos36=%lld, head1_pos36=%lld\n",
                (long long)total_elements, (long long)head0_pos36, (long long)head1_pos36);
        // Since heads 0 and 1 both use same KV head 0 (after GQA), they should produce identical output
        if (head1_pos36 < total_elements) {
            float raw_h0, raw_h1;
            ggml_backend_tensor_get(layer0_attn_out_raw, &raw_h0, head0_pos36 * sizeof(float), sizeof(float));
            ggml_backend_tensor_get(layer0_attn_out_raw, &raw_h1, head1_pos36 * sizeof(float), sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: attn_out_raw head0[36]=%.8f, head1[36]=%.8f, match=%s\n",
                    raw_h0, raw_h1, (std::abs(raw_h0 - raw_h1) < 1e-5f) ? "YES" : "NO");
        }
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_out_raw NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    // Debug: attn_out after permute+cont [head_dim, n_head, n_tokens]
    struct ggml_tensor* layer0_attn_after_permute = ggml_graph_get_tensor(graph, "layer0_attn_after_permute");
    if (layer0_attn_after_permute && layer0_attn_after_permute->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_attn_after_permute, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_after_permute shape=[%lld,%lld,%lld] first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                (long long)layer0_attn_after_permute->ne[0], (long long)layer0_attn_after_permute->ne[1], (long long)layer0_attn_after_permute->ne[2],
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        // Check element at offset 100
        std::vector<float> data100(10);
        ggml_backend_tensor_get(layer0_attn_after_permute, data100.data(), 100 * sizeof(float), 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_after_permute elements [100:110]: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data100[0], data100[1], data100[2], data100[3], data100[4], data100[5], data100[6], data100[7], data100[8], data100[9]);

        // For token 0, heads 0-6 should all have same values (from KV head 0)
        // Shape is [64, 14, 20], so position 36 = (36, 0, 0), position 100 = (36, 1, 0)
        // Both should equal V_kv0[36, 0] for token 0
        float attn_pos36, attn_pos100;
        ggml_backend_tensor_get(layer0_attn_after_permute, &attn_pos36, 36 * sizeof(float), sizeof(float));
        ggml_backend_tensor_get(layer0_attn_after_permute, &attn_pos100, 100 * sizeof(float), sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: attn_after_permute[36] (d36,h0,t0)=%.8f, [100] (d36,h1,t0)=%.8f, match=%s\n",
                attn_pos36, attn_pos100, (std::abs(attn_pos36 - attn_pos100) < 1e-5f) ? "YES" : "NO");
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_after_permute NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    struct ggml_tensor* layer0_before_oproj = ggml_graph_get_tensor(graph, "layer0_before_oproj");
    if (layer0_before_oproj && layer0_before_oproj->buffer) {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_before_oproj shape=[%lld,%lld,%lld,%lld]\n",
                (long long)layer0_before_oproj->ne[0], (long long)layer0_before_oproj->ne[1],
                (long long)layer0_before_oproj->ne[2], (long long)layer0_before_oproj->ne[3]);
        std::vector<float> data(10);
        ggml_backend_tensor_get(layer0_before_oproj, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_before_oproj first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        fflush(stderr);

        // Debug: print wo tensor values to verify the weight data
        if (ctx->model->tensors.layers.size() > 0) {
            auto& layer0_wo = ctx->model->tensors.layers[0].wo;
            if (layer0_wo && layer0_wo->buffer) {
                // Print wo shape
                fprintf(stderr, "[DEBUG] forward_pass: layer0 wo shape=[%lld,%lld]\n",
                        (long long)layer0_wo->ne[0], (long long)layer0_wo->ne[1]);

                // Read first 10 values (these are wo[0:10, 0] in ggml column-major)
                std::vector<float> wo_col0(10);
                ggml_backend_tensor_get(layer0_wo, wo_col0.data(), 0, 10 * sizeof(float));
                fprintf(stderr, "[DEBUG] forward_pass: wo column 0 [0:10] (memory order): %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f\n",
                        wo_col0[0], wo_col0[1], wo_col0[2], wo_col0[3], wo_col0[4],
                        wo_col0[5], wo_col0[6], wo_col0[7], wo_col0[8], wo_col0[9]);

                // Read wo row 0 (these are wo[0, 0:10] which requires stride access)
                // In column-major, row 0 values are at offsets 0, 896, 1792, ...
                std::vector<float> wo_row0(10);
                size_t stride = layer0_wo->ne[0] * sizeof(float);  // 896 * 4 bytes
                for (int i = 0; i < 10; i++) {
                    ggml_backend_tensor_get(layer0_wo, &wo_row0[i], i * stride, sizeof(float));
                }
                fprintf(stderr, "[DEBUG] forward_pass: wo row 0 [0:10] (stride access): %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f\n",
                        wo_row0[0], wo_row0[1], wo_row0[2], wo_row0[3], wo_row0[4],
                        wo_row0[5], wo_row0[6], wo_row0[7], wo_row0[8], wo_row0[9]);

                // Expected PyTorch wo[0, 0:10]: 0.00689697, 0.01092529, 0.01043701, 0.01171875, -0.01257324...
                fprintf(stderr, "[DEBUG] forward_pass: Expected PyTorch wo[0, 0:10]: 0.00689697 0.01092529 0.01043701 0.01171875 -0.01257324 -0.04052734 -0.00585938 -0.01733398 -0.03540039 -0.00296021\n");

                // Manual computation: expected output[0] = sum_k input[k] * pytorch_wo[0, k]
                // pytorch_wo[0, k] = ggml_wo[k, 0] (due to row-major to column-major transpose)
                // ggml_wo[k, 0] is at memory offset k * sizeof(float), which is wo_col0 (memory order)

                // So: output[0] = sum_k input[k] * wo_col0[k]
                double manual_out0_v1 = 0.0;
                for (int k = 0; k < 10; k++) {
                    manual_out0_v1 += data[k] * wo_col0[k];
                }
                fprintf(stderr, "[DEBUG] forward_pass: Manual partial output[0] using wo column 0 (first 10 terms) = %.8f\n", manual_out0_v1);

                // Also test: output[0] = sum_k input[k] * wo_row0[k] (if ggml uses different convention)
                double manual_out0_v2 = 0.0;
                for (int k = 0; k < 10; k++) {
                    manual_out0_v2 += data[k] * wo_row0[k];
                }
                fprintf(stderr, "[DEBUG] forward_pass: Manual partial output[0] using wo row 0 (first 10 terms) = %.8f\n", manual_out0_v2);

                // Full computation for output[0] using all 896 elements
                std::vector<float> input_full(896);
                ggml_backend_tensor_get(layer0_before_oproj, input_full.data(), 0, 896 * sizeof(float));

                std::vector<float> wo_col0_full(896);
                ggml_backend_tensor_get(layer0_wo, wo_col0_full.data(), 0, 896 * sizeof(float));

                // Verify INPUT values at specific positions (THE KEY CHECK!)
                fprintf(stderr, "[DEBUG] forward_pass: C++ input[100:110]: %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f\n",
                        input_full[100], input_full[101], input_full[102], input_full[103], input_full[104],
                        input_full[105], input_full[106], input_full[107], input_full[108], input_full[109]);
                fprintf(stderr, "[DEBUG] forward_pass: Python input[100:110]: -0.01185845 0.00935552 -0.01751706 -0.01777427 -0.01492144 0.00396902 0.00011762 0.00082328 -0.00423377 -0.01285193\n");
                fprintf(stderr, "[DEBUG] forward_pass: C++ input[500:510]: %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f %.8f\n",
                        input_full[500], input_full[501], input_full[502], input_full[503], input_full[504],
                        input_full[505], input_full[506], input_full[507], input_full[508], input_full[509]);
                fprintf(stderr, "[DEBUG] forward_pass: Python input[500:510]: -0.00717101 -0.01702837 0.00539054 -0.03737309 -0.00997916 -0.01208786 -0.05263824 0.02893736 -0.04277842 0.00754921\n");

                // Compute sum of abs values for input
                double input_sum = 0.0;
                for (int k = 0; k < 896; k++) {
                    input_sum += std::abs(input_full[k]);
                }
                fprintf(stderr, "[DEBUG] forward_pass: C++ sum(abs(input)) = %.8f (Python expected: 14.29706669)\n", input_sum);

                double manual_out0_full = 0.0;
                for (int k = 0; k < 896; k++) {
                    manual_out0_full += input_full[k] * wo_col0_full[k];
                }
                fprintf(stderr, "[DEBUG] forward_pass: Manual FULL output[0] = %.8f\n", manual_out0_full);
                fprintf(stderr, "[DEBUG] forward_pass: Expected Python output[0] = -0.01738128\n");

                // Also compute sum of abs values for wo comparison
                double wo_col0_sum = 0.0;
                for (int k = 0; k < 896; k++) {
                    wo_col0_sum += std::abs(wo_col0_full[k]);
                }
                fprintf(stderr, "[DEBUG] forward_pass: sum(abs(wo_col0)) = %.8f (expected 15.80590725)\n", wo_col0_sum);
                fflush(stderr);
            }
        }
    }

    struct ggml_tensor* layer0_attn_raw = ggml_graph_get_tensor(graph, "layer0_attn_raw");
    if (layer0_attn_raw && layer0_attn_raw->buffer) {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_raw shape=[%lld,%lld,%lld,%lld]\n",
                (long long)layer0_attn_raw->ne[0], (long long)layer0_attn_raw->ne[1],
                (long long)layer0_attn_raw->ne[2], (long long)layer0_attn_raw->ne[3]);
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_attn_raw, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_raw first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    }

    struct ggml_tensor* layer0_attn_out = ggml_graph_get_tensor(graph, "layer0_attn_out");
    if (layer0_attn_out && layer0_attn_out->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_attn_out, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_attn_out first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    }

    struct ggml_tensor* layer0_residual2 = ggml_graph_get_tensor(graph, "layer0_residual2");
    if (layer0_residual2 && layer0_residual2->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_residual2, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual2 tensor=%p, data=%p, buffer=%p\n",
                (void*)layer0_residual2, layer0_residual2->data, (void*)layer0_residual2->buffer);
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual2 (before ffn) first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fflush(stderr);
    } else {
        fprintf(stderr, "[DEBUG] forward_pass: layer0_residual2 NOT FOUND or no buffer\n");
        fflush(stderr);
    }

    struct ggml_tensor* layer0_ffn_norm = ggml_graph_get_tensor(graph, "layer0_ffn_norm");
    if (layer0_ffn_norm && layer0_ffn_norm->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_ffn_norm, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_ffn_norm first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fprintf(stderr, "[DEBUG] forward_pass: layer0_ffn_norm tensor=%p, data=%p, buffer=%p\n",
                (void*)layer0_ffn_norm, layer0_ffn_norm->data, (void*)layer0_ffn_norm->buffer);
        fflush(stderr);
    }

    struct ggml_tensor* layer0_ffn_out = ggml_graph_get_tensor(graph, "layer0_ffn_out");
    if (layer0_ffn_out && layer0_ffn_out->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_ffn_out, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_ffn_out first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fprintf(stderr, "[DEBUG] forward_pass: layer0_ffn_out tensor=%p, data=%p, buffer=%p\n",
                (void*)layer0_ffn_out, layer0_ffn_out->data, (void*)layer0_ffn_out->buffer);
        // Compare pointers
        fprintf(stderr, "[DEBUG] forward_pass: ffn_norm == ffn_out? %s, same_buffer? %s, same_data? %s\n",
                layer0_ffn_norm == layer0_ffn_out ? "YES" : "NO",
                layer0_ffn_norm->buffer == layer0_ffn_out->buffer ? "YES" : "NO",
                layer0_ffn_norm->data == layer0_ffn_out->data ? "YES" : "NO");
        fflush(stderr);
    }

    struct ggml_tensor* layer0_output = ggml_graph_get_tensor(graph, "layer0_output");
    if (layer0_output && layer0_output->buffer) {
        std::vector<float> data(5);
        ggml_backend_tensor_get(layer0_output, data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: layer0_output first 5: %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4]);
        fprintf(stderr, "[DEBUG] forward_pass: layer0_output buffer=%p, data=%p\n",
                (void*)layer0_output->buffer, layer0_output->data);
        // Check ggml_add inputs
        if (layer0_output->src[0] && layer0_output->src[0]->buffer) {
            std::vector<float> src0_data(5);
            ggml_backend_tensor_get(layer0_output->src[0], src0_data.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: layer0_output.src[0] (ffn_out) first 5: %.6f %.6f %.6f %.6f %.6f\n",
                    src0_data[0], src0_data[1], src0_data[2], src0_data[3], src0_data[4]);
            fprintf(stderr, "[DEBUG] forward_pass: src[0] buffer=%p, data=%p\n",
                    (void*)layer0_output->src[0]->buffer, layer0_output->src[0]->data);
        }
        if (layer0_output->src[1] && layer0_output->src[1]->buffer) {
            std::vector<float> src1_data(5);
            ggml_backend_tensor_get(layer0_output->src[1], src1_data.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: layer0_output.src[1] (residual) first 5: %.6f %.6f %.6f %.6f %.6f\n",
                    src1_data[0], src1_data[1], src1_data[2], src1_data[3], src1_data[4]);
            fprintf(stderr, "[DEBUG] forward_pass: src[1] buffer=%p, data=%p\n",
                    (void*)layer0_output->src[1]->buffer, layer0_output->src[1]->data);
            // Check for data aliasing
            fprintf(stderr, "[DEBUG] forward_pass: output.data==src[0].data? %s, output.data==src[1].data? %s\n",
                    layer0_output->data == layer0_output->src[0]->data ? "YES" : "NO",
                    layer0_output->data == layer0_output->src[1]->data ? "YES" : "NO");
            // Manually compute expected value
            std::vector<float> src0_data(5);
            ggml_backend_tensor_get(layer0_output->src[0], src0_data.data(), 0, 5 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: EXPECTED layer0_output = src[0]+src[1]: %.6f %.6f %.6f %.6f %.6f\n",
                    src0_data[0]+src1_data[0], src0_data[1]+src1_data[1], src0_data[2]+src1_data[2],
                    src0_data[3]+src1_data[3], src0_data[4]+src1_data[4]);
        } else {
            fprintf(stderr, "[DEBUG] forward_pass: layer0_output.src[1] has NO buffer!\n");
        }
        fflush(stderr);
    }

    // Debug: Check last layer output (before final_norm)
    struct ggml_tensor* last_layer_output = ggml_graph_get_tensor(graph, "last_layer_output");
    if (last_layer_output && last_layer_output->buffer) {
        std::vector<float> data(10);
        ggml_backend_tensor_get(last_layer_output, data.data(), 0, 10 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: last_layer_output first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9]);
        // Calculate max absolute value
        float max_abs = 0.0f;
        size_t n_embd = last_layer_output->ne[0];
        std::vector<float> all_data(n_embd);
        ggml_backend_tensor_get(last_layer_output, all_data.data(), 0, n_embd * sizeof(float));
        for (size_t i = 0; i < n_embd; i++) {
            if (std::abs(all_data[i]) > max_abs) max_abs = std::abs(all_data[i]);
        }
        fprintf(stderr, "[DEBUG] forward_pass: last_layer_output max_abs=%.6f (expected < 10)\n", max_abs);
        fflush(stderr);
    }

    // Debug: Check final_norm output (hidden state before lm_head)
    struct ggml_tensor* final_norm_last = ggml_graph_get_tensor(graph, "final_norm");
    if (final_norm_last && final_norm_last->buffer) {
        // Read last token's hidden state (position n_tokens-1)
        size_t hidden_dim = final_norm_last->ne[0];  // 896
        size_t last_token_offset = (n_tokens - 1) * hidden_dim * sizeof(float);
        std::vector<float> hidden_data(5);
        ggml_backend_tensor_get(final_norm_last, hidden_data.data(), last_token_offset, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: final_norm (last token) first 5: %.6f %.6f %.6f %.6f %.6f\n",
                hidden_data[0], hidden_data[1], hidden_data[2], hidden_data[3], hidden_data[4]);
        // Read hidden[10:15] to compare with Python
        std::vector<float> hidden_data2(5);
        ggml_backend_tensor_get(final_norm_last, hidden_data2.data(), last_token_offset + 10 * sizeof(float), 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] forward_pass: final_norm (last token) [10:15]: %.6f %.6f %.6f %.6f %.6f\n",
                hidden_data2[0], hidden_data2[1], hidden_data2[2], hidden_data2[3], hidden_data2[4]);
        fflush(stderr);

        // Debug: Check final_norm input (last layer output)
        if (final_norm_last->src[0] && final_norm_last->src[0]->buffer) {
            struct ggml_tensor* last_layer = final_norm_last->src[0];
            std::vector<float> input_data(10);
            ggml_backend_tensor_get(last_layer, input_data.data(), last_token_offset, 10 * sizeof(float));
            fprintf(stderr, "[DEBUG] forward_pass: last_layer (final_norm input) first 10: %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f %.6f\n",
                    input_data[0], input_data[1], input_data[2], input_data[3], input_data[4],
                    input_data[5], input_data[6], input_data[7], input_data[8], input_data[9]);
            // Calculate max absolute value
            float max_abs = 0.0f;
            std::vector<float> all_data(hidden_dim);
            ggml_backend_tensor_get(last_layer, all_data.data(), last_token_offset, hidden_dim * sizeof(float));
            for (size_t i = 0; i < hidden_dim; i++) {
                if (std::abs(all_data[i]) > max_abs) max_abs = std::abs(all_data[i]);
            }
            fprintf(stderr, "[DEBUG] forward_pass: last_layer max_abs=%.6f (expected < 10)\n", max_abs);
            fflush(stderr);
        }
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
        // CRITICAL DEBUG: Check logits immediately after copy, BEFORE freeing allocr
        fprintf(stderr, "[DEBUG] forward_pass: IMMEDIATELY after tensor_get, logits ptr=%p\n", (void*)logits);
        fprintf(stderr, "[DEBUG] forward_pass: IMMEDIATELY after tensor_get, logits[0:5]=%.4f %.4f %.4f %.4f %.4f\n",
                logits[0], logits[1], logits[2], logits[3], logits[4]);
        fprintf(stderr, "[DEBUG] forward_pass: IMMEDIATELY after tensor_get, logits[198]=%.4f\n", logits[198]);
        // Find argmax immediately
        float max_val = logits[0];
        int max_idx = 0;
        for (int i = 1; i < (int)hparams.n_vocab; i++) {
            if (logits[i] > max_val) {
                max_val = logits[i];
                max_idx = i;
            }
        }
        fprintf(stderr, "[DEBUG] forward_pass: IMMEDIATELY after tensor_get, argmax=%d, max_val=%.4f\n", max_idx, max_val);
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

    // CRITICAL DEBUG: Check if freeing allocr corrupted logits
    fprintf(stderr, "[DEBUG] forward_pass: AFTER ggml_gallocr_free, logits ptr=%p\n", (void*)logits);
    fprintf(stderr, "[DEBUG] forward_pass: AFTER ggml_gallocr_free, logits[0:5]=%.4f %.4f %.4f %.4f %.4f\n",
            logits[0], logits[1], logits[2], logits[3], logits[4]);
    fprintf(stderr, "[DEBUG] forward_pass: AFTER ggml_gallocr_free, logits[198]=%.4f\n", logits[198]);
    fflush(stderr);

    if (!logits_copied) {
        error = "Logits tensor has no data";
        return false;
    }

    // Debug: check first few logit values
    fprintf(stderr, "[DEBUG] forward_pass: first 5 logits: %.4f %.4f %.4f %.4f %.4f\n",
            logits[0], logits[1], logits[2], logits[3], logits[4]);
    // Debug: check specific token logits (compare to Python)
    // Python: logits[12]=5.78, logits[9707]=14.80, logits[39814]=7.82
    fprintf(stderr, "[DEBUG] forward_pass: logits[12 '-']=%.4f, logits[9707 'Hello']=%.4f, logits[39814 'Sure']=%.4f\n",
            logits[12], logits[9707], logits[39814]);
    // Debug: find top 5 tokens
    std::vector<std::pair<float, int>> top_tokens;
    for (int i = 0; i < (int)hparams.n_vocab; i++) {
        top_tokens.push_back({logits[i], i});
    }
    std::partial_sort(top_tokens.begin(), top_tokens.begin() + 5, top_tokens.end(),
                      [](const auto& a, const auto& b) { return a.first > b.first; });
    fprintf(stderr, "[DEBUG] forward_pass: TOP 5 tokens: [%d]=%.4f, [%d]=%.4f, [%d]=%.4f, [%d]=%.4f, [%d]=%.4f\n",
            top_tokens[0].second, top_tokens[0].first,
            top_tokens[1].second, top_tokens[1].first,
            top_tokens[2].second, top_tokens[2].first,
            top_tokens[3].second, top_tokens[3].first,
            top_tokens[4].second, top_tokens[4].first);
    fflush(stderr);

    // Update KV cache position
    ctx->kv_used = n_past + n_tokens;

    fprintf(stderr, "[DEBUG] forward_pass: done\n");
    fflush(stderr);

    return true;
}

}  // namespace stcpp
