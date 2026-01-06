#pragma once

#include <cuda_runtime.h>
#include <cuda_bf16.h>
#include <cstddef>

namespace nemotron {
namespace kernels {

// RMS Normalization
// out[i] = (x[i] / rms(x)) * weight[i]
// where rms(x) = sqrt(mean(x^2) + eps)
void rmsNorm(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const __nv_bfloat16* weight,
    float eps,
    size_t batch_size,
    size_t hidden_size,
    cudaStream_t stream = nullptr
);

// SiLU activation (Swish): x * sigmoid(x)
void silu(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    size_t size,
    cudaStream_t stream = nullptr
);

// SiLU with element-wise multiplication: silu(gate) * up
void siluMul(
    __nv_bfloat16* output,
    const __nv_bfloat16* gate,
    const __nv_bfloat16* up,
    size_t size,
    cudaStream_t stream = nullptr
);

// Softmax over last dimension
void softmax(
    float* output,
    const float* input,
    size_t batch_size,
    size_t seq_len,
    cudaStream_t stream = nullptr
);

// Embedding lookup
void embeddingLookup(
    __nv_bfloat16* output,
    const __nv_bfloat16* embed_table,
    const int32_t* token_ids,
    size_t batch_size,
    size_t seq_len,
    size_t hidden_size,
    cudaStream_t stream = nullptr
);

// RoPE (Rotary Position Embedding)
void applyRoPE(
    __nv_bfloat16* q,
    __nv_bfloat16* k,
    size_t batch_size,
    size_t seq_len,
    size_t num_heads,
    size_t num_kv_heads,
    size_t head_dim,
    size_t position_offset,
    float rope_theta,
    cudaStream_t stream = nullptr
);

// Scaled dot-product attention
// attn_output = softmax(Q @ K^T / sqrt(d)) @ V
void scaledDotProductAttention(
    __nv_bfloat16* output,
    const __nv_bfloat16* q,
    const __nv_bfloat16* k,
    const __nv_bfloat16* v,
    size_t batch_size,
    size_t num_heads,
    size_t num_kv_heads,
    size_t seq_len,
    size_t kv_seq_len,
    size_t head_dim,
    float scale,
    size_t position_offset,
    cudaStream_t stream = nullptr
);

// Add residual: output = input + residual
void addResidual(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const __nv_bfloat16* residual,
    size_t size,
    cudaStream_t stream = nullptr
);

// Copy tensor
void copyTensor(
    __nv_bfloat16* dst,
    const __nv_bfloat16* src,
    size_t size,
    cudaStream_t stream = nullptr
);

// Convert BF16 logits to FP32
void bf16ToFloat(
    float* output,
    const __nv_bfloat16* input,
    size_t size,
    cudaStream_t stream = nullptr
);

// Argmax for sampling
int32_t argmax(
    const float* logits,
    size_t vocab_size,
    cudaStream_t stream = nullptr
);

// Top-k sampling (returns token id)
int32_t topKSample(
    const float* logits,
    size_t vocab_size,
    int k,
    float temperature,
    cudaStream_t stream = nullptr
);

}  // namespace kernels
}  // namespace nemotron
