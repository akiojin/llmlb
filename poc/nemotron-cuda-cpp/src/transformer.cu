#include "transformer.h"
#include "cuda_utils.h"
#include <algorithm>

namespace nemotron {

// TransformerLayer implementation

TransformerLayer::TransformerLayer(const ModelConfig& config, cublasHandle_t cublas)
    : config_(config), cublas_(cublas) {
    allocateBuffers(config.max_position_embeddings);
}

void TransformerLayer::allocateBuffers(size_t max_seq_len) {
    size_t hidden_size = config_.hidden_size;
    size_t intermediate_size = config_.intermediate_size;
    size_t num_heads = config_.num_attention_heads;
    size_t num_kv_heads = config_.num_key_value_heads;
    size_t head_dim = config_.head_dim();

    // Attention buffers
    q_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * num_heads * head_dim);
    k_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * num_kv_heads * head_dim);
    v_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * num_kv_heads * head_dim);
    attn_out_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);

    // MLP buffers
    gate_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * intermediate_size);
    up_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * intermediate_size);
    mlp_out_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);

    // Norm buffer
    norm_buf_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);
}

void TransformerLayer::gemm(
    __nv_bfloat16* C,
    const __nv_bfloat16* A,
    const __nv_bfloat16* B,
    size_t M, size_t N, size_t K,
    bool trans_a, bool trans_b,
    cudaStream_t stream
) {
    // cuBLAS uses column-major, so we compute C^T = B^T @ A^T
    // which gives us row-major C = A @ B

    cublasOperation_t op_a = trans_b ? CUBLAS_OP_T : CUBLAS_OP_N;
    cublasOperation_t op_b = trans_a ? CUBLAS_OP_T : CUBLAS_OP_N;

    int lda = trans_b ? static_cast<int>(K) : static_cast<int>(N);
    int ldb = trans_a ? static_cast<int>(M) : static_cast<int>(K);
    int ldc = static_cast<int>(N);

    float alpha = 1.0f;
    float beta = 0.0f;

    CUBLAS_CHECK(cublasSetStream(cublas_, stream));
    CUBLAS_CHECK(cublasGemmEx(
        cublas_,
        op_a, op_b,
        static_cast<int>(N), static_cast<int>(M), static_cast<int>(K),
        &alpha,
        B, CUDA_R_16BF, lda,
        A, CUDA_R_16BF, ldb,
        &beta,
        C, CUDA_R_16BF, ldc,
        CUBLAS_COMPUTE_32F,
        CUBLAS_GEMM_DEFAULT
    ));
}

void TransformerLayer::selfAttention(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const TransformerLayerWeights& weights,
    size_t seq_len,
    size_t position_offset,
    __nv_bfloat16* key_cache,
    __nv_bfloat16* value_cache,
    size_t kv_seq_len,
    cudaStream_t stream
) {
    size_t hidden_size = config_.hidden_size;
    size_t num_heads = config_.num_attention_heads;
    size_t num_kv_heads = config_.num_key_value_heads;
    size_t head_dim = config_.head_dim();

    // Q projection: [seq_len, hidden] @ [hidden, hidden] -> [seq_len, hidden]
    gemm(q_buf_.get(),
         input,
         static_cast<const __nv_bfloat16*>(weights.q_proj.data),
         seq_len, hidden_size, hidden_size,
         false, true, stream);

    // K projection: [seq_len, hidden] @ [hidden, kv_dim] -> [seq_len, kv_dim]
    size_t kv_dim = num_kv_heads * head_dim;
    gemm(k_buf_.get(),
         input,
         static_cast<const __nv_bfloat16*>(weights.k_proj.data),
         seq_len, kv_dim, hidden_size,
         false, true, stream);

    // V projection
    gemm(v_buf_.get(),
         input,
         static_cast<const __nv_bfloat16*>(weights.v_proj.data),
         seq_len, kv_dim, hidden_size,
         false, true, stream);

    // Apply RoPE to Q and K
    kernels::applyRoPE(
        q_buf_.get(), k_buf_.get(),
        1, seq_len, num_heads, num_kv_heads, head_dim,
        position_offset, config_.rope_theta, stream
    );

    // Update KV cache
    // Copy new K and V to cache at position_offset
    CUDA_CHECK(cudaMemcpyAsync(
        key_cache + position_offset * kv_dim,
        k_buf_.get(),
        seq_len * kv_dim * sizeof(__nv_bfloat16),
        cudaMemcpyDeviceToDevice, stream
    ));
    CUDA_CHECK(cudaMemcpyAsync(
        value_cache + position_offset * kv_dim,
        v_buf_.get(),
        seq_len * kv_dim * sizeof(__nv_bfloat16),
        cudaMemcpyDeviceToDevice, stream
    ));

    // Attention: Q @ K^T / sqrt(d) -> softmax -> @ V
    float scale = 1.0f / sqrtf(static_cast<float>(head_dim));
    kernels::scaledDotProductAttention(
        attn_out_buf_.get(),
        q_buf_.get(),
        key_cache,
        value_cache,
        1, num_heads, num_kv_heads,
        seq_len, kv_seq_len,
        head_dim, scale, position_offset, stream
    );

    // Output projection: [seq_len, hidden] @ [hidden, hidden] -> [seq_len, hidden]
    gemm(output,
         attn_out_buf_.get(),
         static_cast<const __nv_bfloat16*>(weights.o_proj.data),
         seq_len, hidden_size, hidden_size,
         false, true, stream);
}

void TransformerLayer::mlp(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const TransformerLayerWeights& weights,
    size_t seq_len,
    cudaStream_t stream
) {
    size_t hidden_size = config_.hidden_size;
    size_t intermediate_size = config_.intermediate_size;

    // Gate projection: [seq_len, hidden] @ [hidden, intermediate] -> [seq_len, intermediate]
    gemm(gate_buf_.get(),
         input,
         static_cast<const __nv_bfloat16*>(weights.gate_proj.data),
         seq_len, intermediate_size, hidden_size,
         false, true, stream);

    // Up projection
    gemm(up_buf_.get(),
         input,
         static_cast<const __nv_bfloat16*>(weights.up_proj.data),
         seq_len, intermediate_size, hidden_size,
         false, true, stream);

    // SiLU(gate) * up
    kernels::siluMul(gate_buf_.get(), gate_buf_.get(), up_buf_.get(),
                     seq_len * intermediate_size, stream);

    // Down projection: [seq_len, intermediate] @ [intermediate, hidden] -> [seq_len, hidden]
    gemm(output,
         gate_buf_.get(),
         static_cast<const __nv_bfloat16*>(weights.down_proj.data),
         seq_len, hidden_size, intermediate_size,
         false, true, stream);
}

void TransformerLayer::forward(
    __nv_bfloat16* hidden_states,
    __nv_bfloat16* residual,
    const TransformerLayerWeights& weights,
    size_t seq_len,
    size_t position_offset,
    __nv_bfloat16* key_cache,
    __nv_bfloat16* value_cache,
    size_t kv_seq_len,
    cudaStream_t stream
) {
    size_t hidden_size = config_.hidden_size;

    // Save residual
    kernels::copyTensor(residual, hidden_states, seq_len * hidden_size, stream);

    // Input LayerNorm
    kernels::rmsNorm(
        norm_buf_.get(), hidden_states,
        static_cast<const __nv_bfloat16*>(weights.input_layernorm.data),
        config_.rms_norm_eps, seq_len, hidden_size, stream
    );

    // Self-Attention
    selfAttention(
        hidden_states, norm_buf_.get(), weights,
        seq_len, position_offset, key_cache, value_cache, kv_seq_len, stream
    );

    // Residual add
    kernels::addResidual(hidden_states, hidden_states, residual,
                         seq_len * hidden_size, stream);

    // Save residual for MLP
    kernels::copyTensor(residual, hidden_states, seq_len * hidden_size, stream);

    // Post-attention LayerNorm
    kernels::rmsNorm(
        norm_buf_.get(), hidden_states,
        static_cast<const __nv_bfloat16*>(weights.post_attention_layernorm.data),
        config_.rms_norm_eps, seq_len, hidden_size, stream
    );

    // MLP
    mlp(mlp_out_buf_.get(), norm_buf_.get(), weights, seq_len, stream);

    // Residual add
    kernels::addResidual(hidden_states, mlp_out_buf_.get(), residual,
                         seq_len * hidden_size, stream);
}

// TransformerModel implementation

TransformerModel::TransformerModel(const ModelConfig& config, CudaModelManager& cuda_manager)
    : config_(config), cuda_manager_(cuda_manager) {
    // Create transformer layers
    for (size_t i = 0; i < config.num_hidden_layers; ++i) {
        layers_.push_back(std::make_unique<TransformerLayer>(
            config, cuda_manager.getCublasHandle()));
    }

    allocateBuffers(config.max_position_embeddings);
    LOG_INFO("TransformerModel initialized with " << config.num_hidden_layers << " layers");
}

void TransformerModel::allocateBuffers(size_t max_seq_len) {
    size_t hidden_size = config_.hidden_size;
    size_t num_kv_heads = config_.num_key_value_heads;
    size_t head_dim = config_.head_dim();
    size_t kv_dim = num_kv_heads * head_dim;

    // Activation buffers
    hidden_states_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);
    residual_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);
    token_ids_gpu_ = CudaBuffer<int32_t>(max_seq_len);
    final_norm_out_ = CudaBuffer<__nv_bfloat16>(max_seq_len * hidden_size);
    logits_f32_ = CudaBuffer<float>(config_.vocab_size);

    // KV cache for each layer
    key_cache_.resize(config_.num_hidden_layers);
    value_cache_.resize(config_.num_hidden_layers);
    for (size_t i = 0; i < config_.num_hidden_layers; ++i) {
        key_cache_[i] = CudaBuffer<__nv_bfloat16>(max_seq_len * kv_dim);
        value_cache_[i] = CudaBuffer<__nv_bfloat16>(max_seq_len * kv_dim);
    }
}

void TransformerModel::resetCache() {
    cache_seq_len_ = 0;
}

void TransformerModel::forward(
    float* logits,
    const int32_t* token_ids,
    size_t seq_len,
    size_t position_offset,
    cudaStream_t stream
) {
    const ModelWeights& weights = cuda_manager_.getWeights();
    size_t hidden_size = config_.hidden_size;

    // Copy token IDs to GPU
    token_ids_gpu_.copyFromHost(token_ids, seq_len);

    // Embedding lookup
    kernels::embeddingLookup(
        hidden_states_.get(),
        static_cast<const __nv_bfloat16*>(weights.embed_tokens.data),
        token_ids_gpu_.get(),
        1, seq_len, hidden_size, stream
    );

    // Update KV cache length
    size_t kv_seq_len = position_offset + seq_len;

    // Forward through transformer layers
    for (size_t i = 0; i < config_.num_hidden_layers; ++i) {
        layers_[i]->forward(
            hidden_states_.get(),
            residual_.get(),
            weights.layers[i],
            seq_len,
            position_offset,
            key_cache_[i].get(),
            value_cache_[i].get(),
            kv_seq_len,
            stream
        );
    }

    // Final RMSNorm
    kernels::rmsNorm(
        final_norm_out_.get(), hidden_states_.get(),
        static_cast<const __nv_bfloat16*>(weights.final_norm.data),
        config_.rms_norm_eps, seq_len, hidden_size, stream
    );

    // LM Head: only compute logits for last token
    const __nv_bfloat16* last_hidden = final_norm_out_.get() + (seq_len - 1) * hidden_size;

    // Use embed_tokens as lm_head if tied (common in Llama-style models)
    const __nv_bfloat16* lm_head_weights = weights.lm_head.data ?
        static_cast<const __nv_bfloat16*>(weights.lm_head.data) :
        static_cast<const __nv_bfloat16*>(weights.embed_tokens.data);

    // Temporary buffer for BF16 logits
    CudaBuffer<__nv_bfloat16> logits_bf16(config_.vocab_size);

    // GEMM for logits: [1, hidden] @ [hidden, vocab] -> [1, vocab]
    float alpha = 1.0f, beta = 0.0f;
    CUBLAS_CHECK(cublasSetStream(cuda_manager_.getCublasHandle(), stream));
    CUBLAS_CHECK(cublasGemmEx(
        cuda_manager_.getCublasHandle(),
        CUBLAS_OP_T, CUBLAS_OP_N,
        static_cast<int>(config_.vocab_size), 1, static_cast<int>(hidden_size),
        &alpha,
        lm_head_weights, CUDA_R_16BF, static_cast<int>(hidden_size),
        last_hidden, CUDA_R_16BF, static_cast<int>(hidden_size),
        &beta,
        logits_bf16.get(), CUDA_R_16BF, static_cast<int>(config_.vocab_size),
        CUBLAS_COMPUTE_32F,
        CUBLAS_GEMM_DEFAULT
    ));

    // Convert BF16 logits to FP32
    kernels::bf16ToFloat(logits, logits_bf16.get(), config_.vocab_size, stream);

    // Update cache length
    cache_seq_len_ = kv_seq_len;
}

}  // namespace nemotron
