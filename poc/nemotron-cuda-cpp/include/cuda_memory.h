#pragma once

#include <memory>
#include <unordered_map>
#include <vector>
#include "config.h"
#include "cuda_utils.h"
#include "model_config.h"
#include "safetensors_loader.h"

namespace nemotron {

// Forward declarations
struct TransformerLayer;

// GPU tensor with shape info
struct GpuTensor {
    void* data = nullptr;
    DType dtype = DType::BF16;
    Shape shape;
    size_t size_bytes = 0;

    ~GpuTensor();
    GpuTensor() = default;
    GpuTensor(const GpuTensor&) = delete;
    GpuTensor& operator=(const GpuTensor&) = delete;
    GpuTensor(GpuTensor&& other) noexcept;
    GpuTensor& operator=(GpuTensor&& other) noexcept;

    // Allocate and copy from host
    void allocateAndCopy(const void* host_data, size_t bytes, DType dt, const Shape& sh);

    // Get element count
    size_t numElements() const;
};

// Single transformer layer weights on GPU
struct TransformerLayerWeights {
    GpuTensor input_layernorm;
    GpuTensor post_attention_layernorm;
    GpuTensor q_proj;
    GpuTensor k_proj;
    GpuTensor v_proj;
    GpuTensor o_proj;
    GpuTensor gate_proj;
    GpuTensor up_proj;
    GpuTensor down_proj;
};

// Complete model weights on GPU
struct ModelWeights {
    GpuTensor embed_tokens;
    GpuTensor lm_head;
    GpuTensor final_norm;
    std::vector<TransformerLayerWeights> layers;
};

// GPU model manager
class CudaModelManager {
public:
    CudaModelManager() = default;
    ~CudaModelManager() = default;

    CudaModelManager(const CudaModelManager&) = delete;
    CudaModelManager& operator=(const CudaModelManager&) = delete;

    // Initialize CUDA device
    void initDevice(int device_id = 0);

    // Load model weights to GPU
    void loadWeights(const SafetensorsLoader& loader, const ModelConfig& config);

    // Get model weights
    const ModelWeights& getWeights() const { return weights_; }
    ModelWeights& getWeights() { return weights_; }

    // Get cuBLAS handle
    cublasHandle_t getCublasHandle() const { return cublas_handle_.get(); }

    // Get device info
    int getDeviceId() const { return device_id_; }
    size_t getTotalMemory() const { return total_memory_; }
    size_t getUsedMemory() const { return used_memory_; }

private:
    int device_id_ = 0;
    size_t total_memory_ = 0;
    size_t used_memory_ = 0;
    CublasHandle cublas_handle_;
    ModelWeights weights_;

    void loadTensor(GpuTensor& gpu_tensor, const SafetensorsLoader& loader,
                   const std::string& name);
};

// Inference buffers (activations, KV cache)
class InferenceBuffers {
public:
    InferenceBuffers() = default;

    // Allocate buffers for given config and batch/seq size
    void allocate(const ModelConfig& config, size_t batch_size, size_t max_seq_len);

    // Get buffers
    void* getHiddenStates() { return hidden_states_.get(); }
    void* getResidual() { return residual_.get(); }
    void* getAttnOutput() { return attn_output_.get(); }
    void* getMlpOutput() { return mlp_output_.get(); }
    void* getLogits() { return logits_.get(); }

    // KV cache for each layer
    void* getKeyCache(size_t layer) { return key_cache_[layer].get(); }
    void* getValueCache(size_t layer) { return value_cache_[layer].get(); }

    // Query/Key/Value temporary buffers
    void* getQ() { return q_.get(); }
    void* getK() { return k_.get(); }
    void* getV() { return v_.get(); }

private:
    CudaBuffer<__nv_bfloat16> hidden_states_;
    CudaBuffer<__nv_bfloat16> residual_;
    CudaBuffer<__nv_bfloat16> attn_output_;
    CudaBuffer<__nv_bfloat16> mlp_output_;
    CudaBuffer<float> logits_;

    // QKV buffers
    CudaBuffer<__nv_bfloat16> q_;
    CudaBuffer<__nv_bfloat16> k_;
    CudaBuffer<__nv_bfloat16> v_;

    // KV cache per layer
    std::vector<CudaBuffer<__nv_bfloat16>> key_cache_;
    std::vector<CudaBuffer<__nv_bfloat16>> value_cache_;
};

}  // namespace nemotron
