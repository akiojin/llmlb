#include "kernels.h"
#include "cuda_utils.h"

namespace nemotron {
namespace kernels {

__device__ __forceinline__ float sigmoid(float x) {
    return 1.0f / (1.0f + expf(-x));
}

__global__ void siluKernel(
    __nv_bfloat16* __restrict__ output,
    const __nv_bfloat16* __restrict__ input,
    size_t size
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < size) {
        float x = __bfloat162float(input[idx]);
        float result = x * sigmoid(x);
        output[idx] = __float2bfloat16(result);
    }
}

__global__ void siluMulKernel(
    __nv_bfloat16* __restrict__ output,
    const __nv_bfloat16* __restrict__ gate,
    const __nv_bfloat16* __restrict__ up,
    size_t size
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < size) {
        float g = __bfloat162float(gate[idx]);
        float u = __bfloat162float(up[idx]);
        float silu_g = g * sigmoid(g);
        output[idx] = __float2bfloat16(silu_g * u);
    }
}

void silu(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    size_t size,
    cudaStream_t stream
) {
    constexpr size_t blockSize = 256;
    size_t gridSize = (size + blockSize - 1) / blockSize;

    siluKernel<<<gridSize, blockSize, 0, stream>>>(output, input, size);
    CUDA_KERNEL_CHECK();
}

void siluMul(
    __nv_bfloat16* output,
    const __nv_bfloat16* gate,
    const __nv_bfloat16* up,
    size_t size,
    cudaStream_t stream
) {
    constexpr size_t blockSize = 256;
    size_t gridSize = (size + blockSize - 1) / blockSize;

    siluMulKernel<<<gridSize, blockSize, 0, stream>>>(output, gate, up, size);
    CUDA_KERNEL_CHECK();
}

// Residual add
__global__ void addResidualKernel(
    __nv_bfloat16* __restrict__ output,
    const __nv_bfloat16* __restrict__ input,
    const __nv_bfloat16* __restrict__ residual,
    size_t size
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < size) {
        float a = __bfloat162float(input[idx]);
        float b = __bfloat162float(residual[idx]);
        output[idx] = __float2bfloat16(a + b);
    }
}

void addResidual(
    __nv_bfloat16* output,
    const __nv_bfloat16* input,
    const __nv_bfloat16* residual,
    size_t size,
    cudaStream_t stream
) {
    constexpr size_t blockSize = 256;
    size_t gridSize = (size + blockSize - 1) / blockSize;

    addResidualKernel<<<gridSize, blockSize, 0, stream>>>(
        output, input, residual, size
    );
    CUDA_KERNEL_CHECK();
}

// Copy tensor
__global__ void copyTensorKernel(
    __nv_bfloat16* __restrict__ dst,
    const __nv_bfloat16* __restrict__ src,
    size_t size
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < size) {
        dst[idx] = src[idx];
    }
}

void copyTensor(
    __nv_bfloat16* dst,
    const __nv_bfloat16* src,
    size_t size,
    cudaStream_t stream
) {
    constexpr size_t blockSize = 256;
    size_t gridSize = (size + blockSize - 1) / blockSize;

    copyTensorKernel<<<gridSize, blockSize, 0, stream>>>(dst, src, size);
    CUDA_KERNEL_CHECK();
}

// BF16 to Float conversion
__global__ void bf16ToFloatKernel(
    float* __restrict__ output,
    const __nv_bfloat16* __restrict__ input,
    size_t size
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < size) {
        output[idx] = __bfloat162float(input[idx]);
    }
}

void bf16ToFloat(
    float* output,
    const __nv_bfloat16* input,
    size_t size,
    cudaStream_t stream
) {
    constexpr size_t blockSize = 256;
    size_t gridSize = (size + blockSize - 1) / blockSize;

    bf16ToFloatKernel<<<gridSize, blockSize, 0, stream>>>(output, input, size);
    CUDA_KERNEL_CHECK();
}

}  // namespace kernels
}  // namespace nemotron
