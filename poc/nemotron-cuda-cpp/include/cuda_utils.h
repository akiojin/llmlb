#pragma once

#include <cuda_runtime.h>
#include <cublas_v2.h>
#include <cuda_fp16.h>
#include <cuda_bf16.h>
#include <sstream>
#include <string>
#include "config.h"

namespace nemotron {

// CUDA error checking macro
#define CUDA_CHECK(call)                                                       \
    do {                                                                       \
        cudaError_t err = (call);                                              \
        if (err != cudaSuccess) {                                              \
            std::ostringstream oss;                                            \
            oss << "CUDA error at " << __FILE__ << ":" << __LINE__ << " - "    \
                << cudaGetErrorString(err) << " (" << static_cast<int>(err)    \
                << ")";                                                        \
            throw CudaError(oss.str());                                        \
        }                                                                      \
    } while (0)

// cuBLAS error checking macro
#define CUBLAS_CHECK(call)                                                     \
    do {                                                                       \
        cublasStatus_t status = (call);                                        \
        if (status != CUBLAS_STATUS_SUCCESS) {                                 \
            std::ostringstream oss;                                            \
            oss << "cuBLAS error at " << __FILE__ << ":" << __LINE__           \
                << " - status " << static_cast<int>(status);                   \
            throw CudaError(oss.str());                                        \
        }                                                                      \
    } while (0)

// CUDA kernel launch error check
#define CUDA_KERNEL_CHECK()                                                    \
    do {                                                                       \
        cudaError_t err = cudaGetLastError();                                  \
        if (err != cudaSuccess) {                                              \
            std::ostringstream oss;                                            \
            oss << "CUDA kernel error at " << __FILE__ << ":" << __LINE__      \
                << " - " << cudaGetErrorString(err);                           \
            throw CudaError(oss.str());                                        \
        }                                                                      \
    } while (0)

// cuBLAS handle wrapper (RAII)
class CublasHandle {
public:
    CublasHandle() {
        CUBLAS_CHECK(cublasCreate(&handle_));
    }
    ~CublasHandle() {
        if (handle_) {
            cublasDestroy(handle_);
        }
    }
    CublasHandle(const CublasHandle&) = delete;
    CublasHandle& operator=(const CublasHandle&) = delete;
    CublasHandle(CublasHandle&& other) noexcept : handle_(other.handle_) {
        other.handle_ = nullptr;
    }
    CublasHandle& operator=(CublasHandle&& other) noexcept {
        if (this != &other) {
            if (handle_) cublasDestroy(handle_);
            handle_ = other.handle_;
            other.handle_ = nullptr;
        }
        return *this;
    }
    cublasHandle_t get() const { return handle_; }
private:
    cublasHandle_t handle_ = nullptr;
};

// CUDA memory RAII wrapper
template<typename T>
class CudaBuffer {
public:
    CudaBuffer() = default;
    explicit CudaBuffer(size_t count) : count_(count) {
        if (count > 0) {
            CUDA_CHECK(cudaMalloc(&ptr_, count * sizeof(T)));
        }
    }
    ~CudaBuffer() {
        if (ptr_) {
            cudaFree(ptr_);
        }
    }
    CudaBuffer(const CudaBuffer&) = delete;
    CudaBuffer& operator=(const CudaBuffer&) = delete;
    CudaBuffer(CudaBuffer&& other) noexcept
        : ptr_(other.ptr_), count_(other.count_) {
        other.ptr_ = nullptr;
        other.count_ = 0;
    }
    CudaBuffer& operator=(CudaBuffer&& other) noexcept {
        if (this != &other) {
            if (ptr_) cudaFree(ptr_);
            ptr_ = other.ptr_;
            count_ = other.count_;
            other.ptr_ = nullptr;
            other.count_ = 0;
        }
        return *this;
    }

    T* get() { return ptr_; }
    const T* get() const { return ptr_; }
    size_t size() const { return count_; }
    size_t bytes() const { return count_ * sizeof(T); }

    void copyFromHost(const T* src, size_t count) {
        CUDA_CHECK(cudaMemcpy(ptr_, src, count * sizeof(T), cudaMemcpyHostToDevice));
    }
    void copyToHost(T* dst, size_t count) const {
        CUDA_CHECK(cudaMemcpy(dst, ptr_, count * sizeof(T), cudaMemcpyDeviceToHost));
    }

private:
    T* ptr_ = nullptr;
    size_t count_ = 0;
};

// Device info query
inline bool checkCudaAvailable() {
    int deviceCount = 0;
    cudaError_t err = cudaGetDeviceCount(&deviceCount);
    return (err == cudaSuccess && deviceCount > 0);
}

inline void printDeviceInfo(int device = 0) {
    cudaDeviceProp prop;
    CUDA_CHECK(cudaGetDeviceProperties(&prop, device));
    LOG_INFO("CUDA Device: " << prop.name);
    LOG_INFO("  Compute Capability: " << prop.major << "." << prop.minor);
    LOG_INFO("  Total Memory: " << (prop.totalGlobalMem / (1024*1024)) << " MB");
    LOG_INFO("  SM Count: " << prop.multiProcessorCount);
}

// Common kernel configurations
inline dim3 getBlockDim1D(size_t n, size_t blockSize = 256) {
    return dim3(static_cast<unsigned int>(blockSize));
}

inline dim3 getGridDim1D(size_t n, size_t blockSize = 256) {
    return dim3(static_cast<unsigned int>((n + blockSize - 1) / blockSize));
}

}  // namespace nemotron
