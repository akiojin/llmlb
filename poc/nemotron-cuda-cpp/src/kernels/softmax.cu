#include "kernels.h"
#include "cuda_utils.h"
#include <cfloat>

namespace nemotron {
namespace kernels {

// Warp reduction for max
__device__ __forceinline__ float warpReduceMax(float val) {
    #pragma unroll
    for (int offset = 16; offset > 0; offset /= 2) {
        val = fmaxf(val, __shfl_down_sync(0xffffffff, val, offset));
    }
    return val;
}

// Block reduction for max
__device__ float blockReduceMax(float val) {
    __shared__ float shared[32];
    int lane = threadIdx.x % 32;
    int wid = threadIdx.x / 32;

    val = warpReduceMax(val);

    if (lane == 0) shared[wid] = val;
    __syncthreads();

    val = (threadIdx.x < blockDim.x / 32) ? shared[lane] : -FLT_MAX;
    if (wid == 0) val = warpReduceMax(val);

    return val;
}

// Warp reduction for sum
__device__ __forceinline__ float warpReduceSumSoftmax(float val) {
    #pragma unroll
    for (int offset = 16; offset > 0; offset /= 2) {
        val += __shfl_down_sync(0xffffffff, val, offset);
    }
    return val;
}

// Block reduction for sum
__device__ float blockReduceSumSoftmax(float val) {
    __shared__ float shared[32];
    int lane = threadIdx.x % 32;
    int wid = threadIdx.x / 32;

    val = warpReduceSumSoftmax(val);

    if (lane == 0) shared[wid] = val;
    __syncthreads();

    val = (threadIdx.x < blockDim.x / 32) ? shared[lane] : 0.0f;
    if (wid == 0) val = warpReduceSumSoftmax(val);

    return val;
}

__global__ void softmaxKernel(
    float* __restrict__ output,
    const float* __restrict__ input,
    size_t seq_len
) {
    // Each block handles one row
    const size_t row = blockIdx.x;
    const float* row_input = input + row * seq_len;
    float* row_output = output + row * seq_len;

    // Find max for numerical stability
    float max_val = -FLT_MAX;
    for (size_t i = threadIdx.x; i < seq_len; i += blockDim.x) {
        max_val = fmaxf(max_val, row_input[i]);
    }
    max_val = blockReduceMax(max_val);

    __shared__ float s_max;
    if (threadIdx.x == 0) s_max = max_val;
    __syncthreads();
    max_val = s_max;

    // Compute exp(x - max) and sum
    float sum = 0.0f;
    for (size_t i = threadIdx.x; i < seq_len; i += blockDim.x) {
        float val = expf(row_input[i] - max_val);
        row_output[i] = val;
        sum += val;
    }
    sum = blockReduceSumSoftmax(sum);

    __shared__ float s_sum;
    if (threadIdx.x == 0) s_sum = sum;
    __syncthreads();
    sum = s_sum;

    // Normalize
    float inv_sum = 1.0f / sum;
    for (size_t i = threadIdx.x; i < seq_len; i += blockDim.x) {
        row_output[i] *= inv_sum;
    }
}

void softmax(
    float* output,
    const float* input,
    size_t batch_size,
    size_t seq_len,
    cudaStream_t stream
) {
    dim3 grid(batch_size);
    dim3 block(std::min(seq_len, size_t(1024)));

    softmaxKernel<<<grid, block, 0, stream>>>(output, input, seq_len);
    CUDA_KERNEL_CHECK();
}

// Argmax for greedy sampling
__global__ void argmaxKernel(
    int32_t* __restrict__ result,
    const float* __restrict__ logits,
    size_t vocab_size
) {
    __shared__ float s_max[32];
    __shared__ int32_t s_idx[32];

    int lane = threadIdx.x % 32;
    int wid = threadIdx.x / 32;

    float max_val = -FLT_MAX;
    int32_t max_idx = 0;

    for (size_t i = threadIdx.x; i < vocab_size; i += blockDim.x) {
        float val = logits[i];
        if (val > max_val) {
            max_val = val;
            max_idx = static_cast<int32_t>(i);
        }
    }

    // Warp reduction
    #pragma unroll
    for (int offset = 16; offset > 0; offset /= 2) {
        float other_val = __shfl_down_sync(0xffffffff, max_val, offset);
        int32_t other_idx = __shfl_down_sync(0xffffffff, max_idx, offset);
        if (other_val > max_val) {
            max_val = other_val;
            max_idx = other_idx;
        }
    }

    if (lane == 0) {
        s_max[wid] = max_val;
        s_idx[wid] = max_idx;
    }
    __syncthreads();

    if (wid == 0) {
        max_val = (threadIdx.x < blockDim.x / 32) ? s_max[lane] : -FLT_MAX;
        max_idx = (threadIdx.x < blockDim.x / 32) ? s_idx[lane] : 0;

        #pragma unroll
        for (int offset = 16; offset > 0; offset /= 2) {
            float other_val = __shfl_down_sync(0xffffffff, max_val, offset);
            int32_t other_idx = __shfl_down_sync(0xffffffff, max_idx, offset);
            if (other_val > max_val) {
                max_val = other_val;
                max_idx = other_idx;
            }
        }

        if (lane == 0) {
            *result = max_idx;
        }
    }
}

int32_t argmax(
    const float* logits,
    size_t vocab_size,
    cudaStream_t stream
) {
    int32_t* d_result;
    CUDA_CHECK(cudaMalloc(&d_result, sizeof(int32_t)));

    dim3 block(std::min(vocab_size, size_t(1024)));
    argmaxKernel<<<1, block, 0, stream>>>(d_result, logits, vocab_size);
    CUDA_KERNEL_CHECK();

    int32_t result;
    CUDA_CHECK(cudaMemcpy(&result, d_result, sizeof(int32_t), cudaMemcpyDeviceToHost));
    CUDA_CHECK(cudaFree(d_result));

    return result;
}

// Top-k sampling (simplified: just uses argmax for now)
int32_t topKSample(
    const float* logits,
    size_t vocab_size,
    int k,
    float temperature,
    cudaStream_t stream
) {
    // For PoC, just use greedy argmax
    // TODO: Implement proper top-k sampling with temperature
    (void)k;
    (void)temperature;
    return argmax(logits, vocab_size, stream);
}

}  // namespace kernels
}  // namespace nemotron
