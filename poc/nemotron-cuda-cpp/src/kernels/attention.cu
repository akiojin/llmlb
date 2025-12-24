#include "kernels.h"
#include "cuda_utils.h"
#include <cfloat>

namespace nemotron {
namespace kernels {

// RoPE helper: compute sin/cos for position
__device__ void computeRoPECosSin(
    float& cos_val,
    float& sin_val,
    size_t pos,
    size_t dim_idx,
    size_t head_dim,
    float theta
) {
    float freq = 1.0f / powf(theta, static_cast<float>(dim_idx * 2) / static_cast<float>(head_dim));
    float angle = static_cast<float>(pos) * freq;
    cos_val = cosf(angle);
    sin_val = sinf(angle);
}

__global__ void applyRoPEKernel(
    __nv_bfloat16* __restrict__ q,
    __nv_bfloat16* __restrict__ k,
    size_t seq_len,
    size_t num_q_heads,
    size_t num_kv_heads,
    size_t head_dim,
    size_t position_offset,
    float rope_theta
) {
    // Each thread handles one (position, head, dim_pair)
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    size_t half_head_dim = head_dim / 2;
    size_t total_q = seq_len * num_q_heads * half_head_dim;
    size_t total_kv = seq_len * num_kv_heads * half_head_dim;

    // Process Q
    if (idx < total_q) {
        size_t dim_idx = idx % half_head_dim;
        size_t head_idx = (idx / half_head_dim) % num_q_heads;
        size_t pos = idx / (half_head_dim * num_q_heads);

        size_t base_idx = pos * num_q_heads * head_dim + head_idx * head_dim;
        size_t i0 = base_idx + dim_idx * 2;
        size_t i1 = i0 + 1;

        float cos_val, sin_val;
        computeRoPECosSin(cos_val, sin_val, pos + position_offset, dim_idx, head_dim, rope_theta);

        float x0 = __bfloat162float(q[i0]);
        float x1 = __bfloat162float(q[i1]);

        q[i0] = __float2bfloat16(x0 * cos_val - x1 * sin_val);
        q[i1] = __float2bfloat16(x0 * sin_val + x1 * cos_val);
    }

    // Process K (separate thread range)
    if (idx < total_kv) {
        size_t dim_idx = idx % half_head_dim;
        size_t head_idx = (idx / half_head_dim) % num_kv_heads;
        size_t pos = idx / (half_head_dim * num_kv_heads);

        size_t base_idx = pos * num_kv_heads * head_dim + head_idx * head_dim;
        size_t i0 = base_idx + dim_idx * 2;
        size_t i1 = i0 + 1;

        float cos_val, sin_val;
        computeRoPECosSin(cos_val, sin_val, pos + position_offset, dim_idx, head_dim, rope_theta);

        float x0 = __bfloat162float(k[i0]);
        float x1 = __bfloat162float(k[i1]);

        k[i0] = __float2bfloat16(x0 * cos_val - x1 * sin_val);
        k[i1] = __float2bfloat16(x0 * sin_val + x1 * cos_val);
    }
}

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
    cudaStream_t stream
) {
    (void)batch_size;  // Assume batch_size=1 for PoC

    size_t half_head_dim = head_dim / 2;
    size_t total_q = seq_len * num_heads * half_head_dim;
    size_t total_kv = seq_len * num_kv_heads * half_head_dim;
    size_t total = std::max(total_q, total_kv);

    constexpr size_t blockSize = 256;
    size_t gridSize = (total + blockSize - 1) / blockSize;

    applyRoPEKernel<<<gridSize, blockSize, 0, stream>>>(
        q, k, seq_len, num_heads, num_kv_heads, head_dim, position_offset, rope_theta
    );
    CUDA_KERNEL_CHECK();
}

// Simple attention (not Flash Attention - for PoC)
// This is a naive O(n^2) implementation for correctness
__global__ void computeAttentionScoresKernel(
    float* __restrict__ scores,
    const __nv_bfloat16* __restrict__ q,
    const __nv_bfloat16* __restrict__ k,
    size_t num_heads,
    size_t num_kv_heads,
    size_t seq_len,
    size_t kv_seq_len,
    size_t head_dim,
    float scale
) {
    // scores[h, i, j] = sum_d(q[h, i, d] * k[kv_h, j, d]) * scale
    // where kv_h = h / (num_heads / num_kv_heads)

    size_t h = blockIdx.x;  // head index
    size_t i = blockIdx.y;  // query position
    size_t j = threadIdx.x; // key position

    if (j >= kv_seq_len) return;

    size_t kv_h = h / (num_heads / num_kv_heads);

    float sum = 0.0f;
    for (size_t d = 0; d < head_dim; ++d) {
        float q_val = __bfloat162float(q[i * num_heads * head_dim + h * head_dim + d]);
        float k_val = __bfloat162float(k[j * num_kv_heads * head_dim + kv_h * head_dim + d]);
        sum += q_val * k_val;
    }

    // Apply causal mask
    if (j > i) {
        sum = -FLT_MAX;
    }

    scores[h * seq_len * kv_seq_len + i * kv_seq_len + j] = sum * scale;
}

__global__ void attentionSoftmaxKernel(
    float* __restrict__ scores,
    size_t num_heads,
    size_t seq_len,
    size_t kv_seq_len
) {
    size_t h = blockIdx.x;
    size_t i = blockIdx.y;

    float* row = scores + h * seq_len * kv_seq_len + i * kv_seq_len;

    // Find max
    float max_val = -FLT_MAX;
    for (size_t j = 0; j < kv_seq_len; ++j) {
        max_val = fmaxf(max_val, row[j]);
    }

    // Exp and sum
    float sum = 0.0f;
    for (size_t j = 0; j < kv_seq_len; ++j) {
        row[j] = expf(row[j] - max_val);
        sum += row[j];
    }

    // Normalize
    float inv_sum = 1.0f / sum;
    for (size_t j = 0; j < kv_seq_len; ++j) {
        row[j] *= inv_sum;
    }
}

__global__ void applyAttentionKernel(
    __nv_bfloat16* __restrict__ output,
    const float* __restrict__ scores,
    const __nv_bfloat16* __restrict__ v,
    size_t num_heads,
    size_t num_kv_heads,
    size_t seq_len,
    size_t kv_seq_len,
    size_t head_dim
) {
    size_t h = blockIdx.x;  // head
    size_t i = blockIdx.y;  // query position
    size_t d = threadIdx.x; // dimension

    if (d >= head_dim) return;

    size_t kv_h = h / (num_heads / num_kv_heads);

    float sum = 0.0f;
    for (size_t j = 0; j < kv_seq_len; ++j) {
        float score = scores[h * seq_len * kv_seq_len + i * kv_seq_len + j];
        float v_val = __bfloat162float(v[j * num_kv_heads * head_dim + kv_h * head_dim + d]);
        sum += score * v_val;
    }

    output[i * num_heads * head_dim + h * head_dim + d] = __float2bfloat16(sum);
}

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
    cudaStream_t stream
) {
    (void)batch_size;  // Assume batch_size=1 for PoC

    // Allocate attention scores
    float* d_scores;
    size_t scores_size = num_heads * seq_len * kv_seq_len * sizeof(float);
    CUDA_CHECK(cudaMalloc(&d_scores, scores_size));

    // Compute attention scores: Q @ K^T
    {
        dim3 grid(num_heads, seq_len);
        dim3 block(std::min(kv_seq_len, size_t(1024)));
        computeAttentionScoresKernel<<<grid, block, 0, stream>>>(
            d_scores, q, k, num_heads, num_kv_heads, seq_len, kv_seq_len, head_dim, scale
        );
        CUDA_KERNEL_CHECK();
    }

    // Apply softmax
    {
        dim3 grid(num_heads, seq_len);
        attentionSoftmaxKernel<<<grid, 1, 0, stream>>>(
            d_scores, num_heads, seq_len, kv_seq_len
        );
        CUDA_KERNEL_CHECK();
    }

    // Apply attention to values: scores @ V
    {
        dim3 grid(num_heads, seq_len);
        dim3 block(std::min(head_dim, size_t(256)));
        applyAttentionKernel<<<grid, block, 0, stream>>>(
            output, d_scores, v, num_heads, num_kv_heads, seq_len, kv_seq_len, head_dim
        );
        CUDA_KERNEL_CHECK();
    }

    CUDA_CHECK(cudaFree(d_scores));
}

}  // namespace kernels
}  // namespace nemotron
