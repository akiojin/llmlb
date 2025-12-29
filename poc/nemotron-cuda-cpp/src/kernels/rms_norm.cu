#include "kernels.h"
#include "cuda_utils.h"

namespace nemotron {
namespace kernels {

// Warp reduction for sum
__device__ __forceinline__ float warpReduceSum(float val) {
    #pragma unroll
    for (int offset = 16; offset > 0; offset /= 2) {
        val += __shfl_down_sync(0xffffffff, val, offset);
    }
    return val;
}

// Block reduction for sum
__device__ float blockReduceSum(float val) {
    __shared__ float shared[32];
    int lane = threadIdx.x % 32;
    int wid = threadIdx.x / 32;

    val = warpReduceSum(val);

    if (lane == 0) shared[wid] = val;
    __syncthreads();

    val = (threadIdx.x < blockDim.x / 32) ? shared[lane] : 0.0f;
    if (wid == 0) val = warpReduceSum(val);

    return val;
}

__global__ void rmsNormKernel(
    __nv_bfloat16* __restrict__ output,
    const __nv_bfloat16* __restrict__ input,
    const __nv_bfloat16* __restrict__ weight,
    float eps,
    size_t hidden_size
) {
    // Each block handles one row
    const size_t row = blockIdx.x;
    const __nv_bfloat16* row_input = input + row * hidden_size;
    __nv_bfloat16* row_output = output + row * hidden_size;

    // Compute sum of squares
    float sum_sq = 0.0f;
    for (size_t i = threadIdx.x; i < hidden_size; i += blockDim.x) {
        float val = __bfloat162float(row_input[i]);
        sum_sq += val * val;
    }

    // Reduce within block
    sum_sq = blockReduceSum(sum_sq);

    // Compute RMS
    __shared__ float s_rms;
    if (threadIdx.x == 0) {
        s_rms = rsqrtf(sum_sq / static_cast<float>(hidden_size) + eps);
    }
    __syncthreads();

    float rms = s_rms;

    // Normalize and scale
    for (size_t i = threadIdx.x; i < hidden_size; i += blockDim.x) {
        float val = __bfloat162float(row_input[i]);
        float w = __bfloat162float(weight[i]);
        row_output[i] = __float2bfloat16(val * rms * w);
    }
}

void rmsNorm(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const __nv_bfloat16* weight,
    float eps,
    size_t batch_size,
    size_t hidden_size,
    cudaStream_t stream
) {
    dim3 grid(batch_size);
    dim3 block(std::min(hidden_size, size_t(1024)));

    rmsNormKernel<<<grid, block, 0, stream>>>(
        output, input, weight, eps, hidden_size
    );
    CUDA_KERNEL_CHECK();
}

}  // namespace kernels
}  // namespace nemotron
