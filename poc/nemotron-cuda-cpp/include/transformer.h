#pragma once

#include "config.h"
#include "model_config.h"
#include "cuda_memory.h"
#include "kernels.h"
#include <cuda_bf16.h>

namespace nemotron {

// Transformer layer execution
class TransformerLayer {
public:
    TransformerLayer(const ModelConfig& config, cublasHandle_t cublas);

    // Forward pass for single layer
    void forward(
        __nv_bfloat16* hidden_states,      // [seq_len, hidden_size]
        __nv_bfloat16* residual,           // [seq_len, hidden_size]
        const TransformerLayerWeights& weights,
        size_t seq_len,
        size_t position_offset,            // For KV cache positioning
        __nv_bfloat16* key_cache,          // [max_seq_len, num_kv_heads, head_dim]
        __nv_bfloat16* value_cache,        // [max_seq_len, num_kv_heads, head_dim]
        size_t kv_seq_len,                 // Current KV cache length
        cudaStream_t stream = nullptr
    );

private:
    const ModelConfig& config_;
    cublasHandle_t cublas_;

    // Temporary buffers for attention
    CudaBuffer<__nv_bfloat16> q_buf_;
    CudaBuffer<__nv_bfloat16> k_buf_;
    CudaBuffer<__nv_bfloat16> v_buf_;
    CudaBuffer<__nv_bfloat16> attn_out_buf_;

    // Temporary buffers for MLP
    CudaBuffer<__nv_bfloat16> gate_buf_;
    CudaBuffer<__nv_bfloat16> up_buf_;
    CudaBuffer<__nv_bfloat16> mlp_out_buf_;

    // Temporary buffer for normalized input
    CudaBuffer<__nv_bfloat16> norm_buf_;

    // Allocate buffers
    void allocateBuffers(size_t max_seq_len);

    // Sub-operations
    void selfAttention(
        __nv_bfloat16* output,
        const __nv_bfloat16* input,
        const TransformerLayerWeights& weights,
        size_t seq_len,
        size_t position_offset,
        __nv_bfloat16* key_cache,
        __nv_bfloat16* value_cache,
        size_t kv_seq_len,
        cudaStream_t stream
    );

    void mlp(
        __nv_bfloat16* output,
        const __nv_bfloat16* input,
        const TransformerLayerWeights& weights,
        size_t seq_len,
        cudaStream_t stream
    );

    // GEMM wrapper
    void gemm(
        __nv_bfloat16* C,
        const __nv_bfloat16* A,
        const __nv_bfloat16* B,
        size_t M, size_t N, size_t K,
        bool trans_a, bool trans_b,
        cudaStream_t stream
    );
};

// Full model forward pass
class TransformerModel {
public:
    TransformerModel(const ModelConfig& config, CudaModelManager& cuda_manager);

    // Generate next token logits
    void forward(
        float* logits,                     // [vocab_size] output
        const int32_t* token_ids,          // [seq_len] input
        size_t seq_len,
        size_t position_offset,            // For incremental decoding
        cudaStream_t stream = nullptr
    );

    // Reset KV cache
    void resetCache();

private:
    const ModelConfig& config_;
    CudaModelManager& cuda_manager_;
    std::vector<std::unique_ptr<TransformerLayer>> layers_;

    // KV cache
    std::vector<CudaBuffer<__nv_bfloat16>> key_cache_;
    std::vector<CudaBuffer<__nv_bfloat16>> value_cache_;
    size_t cache_seq_len_ = 0;

    // Buffers
    CudaBuffer<__nv_bfloat16> hidden_states_;
    CudaBuffer<__nv_bfloat16> residual_;
    CudaBuffer<int32_t> token_ids_gpu_;
    CudaBuffer<__nv_bfloat16> final_norm_out_;
    CudaBuffer<float> logits_f32_;

    void allocateBuffers(size_t max_seq_len);
};

}  // namespace nemotron
