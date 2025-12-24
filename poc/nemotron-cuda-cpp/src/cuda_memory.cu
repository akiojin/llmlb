#include "cuda_memory.h"

namespace nemotron {

// GpuTensor implementation

GpuTensor::~GpuTensor() {
    if (data) {
        cudaFree(data);
        data = nullptr;
    }
}

GpuTensor::GpuTensor(GpuTensor&& other) noexcept
    : data(other.data), dtype(other.dtype), shape(std::move(other.shape)),
      size_bytes(other.size_bytes) {
    other.data = nullptr;
    other.size_bytes = 0;
}

GpuTensor& GpuTensor::operator=(GpuTensor&& other) noexcept {
    if (this != &other) {
        if (data) cudaFree(data);
        data = other.data;
        dtype = other.dtype;
        shape = std::move(other.shape);
        size_bytes = other.size_bytes;
        other.data = nullptr;
        other.size_bytes = 0;
    }
    return *this;
}

void GpuTensor::allocateAndCopy(const void* host_data, size_t bytes,
                                 DType dt, const Shape& sh) {
    if (data) {
        cudaFree(data);
    }
    CUDA_CHECK(cudaMalloc(&data, bytes));
    CUDA_CHECK(cudaMemcpy(data, host_data, bytes, cudaMemcpyHostToDevice));
    dtype = dt;
    shape = sh;
    size_bytes = bytes;
}

size_t GpuTensor::numElements() const {
    if (shape.empty()) return 0;
    size_t count = 1;
    for (size_t dim : shape) {
        count *= dim;
    }
    return count;
}

// CudaModelManager implementation

void CudaModelManager::initDevice(int device_id) {
    device_id_ = device_id;

    int device_count = 0;
    CUDA_CHECK(cudaGetDeviceCount(&device_count));

    if (device_count == 0) {
        throw CudaError("No CUDA devices available");
    }

    if (device_id >= device_count) {
        throw CudaError("Invalid device ID: " + std::to_string(device_id) +
                       " (only " + std::to_string(device_count) + " devices available)");
    }

    CUDA_CHECK(cudaSetDevice(device_id));
    printDeviceInfo(device_id);

    cudaDeviceProp prop;
    CUDA_CHECK(cudaGetDeviceProperties(&prop, device_id));
    total_memory_ = prop.totalGlobalMem;

    // Initialize cuBLAS
    cublas_handle_ = CublasHandle();
    LOG_INFO("cuBLAS initialized");
}

void CudaModelManager::loadTensor(GpuTensor& gpu_tensor,
                                   const SafetensorsLoader& loader,
                                   const std::string& name) {
    const TensorInfo* info = loader.getTensor(name);
    if (!info) {
        throw ModelError("Tensor not found: " + name);
    }

    gpu_tensor.allocateAndCopy(info->data, info->data_size, info->dtype, info->shape);
    used_memory_ += info->data_size;

    LOG_DEBUG("Loaded tensor: " << name << " ["
              << info->shape[0] << (info->shape.size() > 1 ? "x" + std::to_string(info->shape[1]) : "")
              << "] " << dtypeToString(info->dtype));
}

void CudaModelManager::loadWeights(const SafetensorsLoader& loader,
                                    const ModelConfig& config) {
    LOG_INFO("Loading model weights to GPU...");
    used_memory_ = 0;

    // Load embedding and output layers
    loadTensor(weights_.embed_tokens, loader, TensorNames::embed_tokens());
    loadTensor(weights_.final_norm, loader, TensorNames::final_norm());

    // lm_head might be tied to embed_tokens
    if (loader.hasTensor(TensorNames::lm_head())) {
        loadTensor(weights_.lm_head, loader, TensorNames::lm_head());
    } else {
        LOG_INFO("lm_head not found, will use tied embeddings");
        // Share with embed_tokens (no additional allocation)
    }

    // Load transformer layers
    weights_.layers.resize(config.num_hidden_layers);

    for (size_t i = 0; i < config.num_hidden_layers; ++i) {
        auto& layer = weights_.layers[i];

        loadTensor(layer.input_layernorm, loader, TensorNames::layer_input_norm(i));
        loadTensor(layer.post_attention_layernorm, loader, TensorNames::layer_post_attn_norm(i));

        loadTensor(layer.q_proj, loader, TensorNames::q_proj(i));
        loadTensor(layer.k_proj, loader, TensorNames::k_proj(i));
        loadTensor(layer.v_proj, loader, TensorNames::v_proj(i));
        loadTensor(layer.o_proj, loader, TensorNames::o_proj(i));

        loadTensor(layer.gate_proj, loader, TensorNames::gate_proj(i));
        loadTensor(layer.up_proj, loader, TensorNames::up_proj(i));
        loadTensor(layer.down_proj, loader, TensorNames::down_proj(i));

        if ((i + 1) % 8 == 0 || i == config.num_hidden_layers - 1) {
            LOG_INFO("  Loaded layer " << (i + 1) << "/" << config.num_hidden_layers);
        }
    }

    LOG_INFO("Model weights loaded: " << (used_memory_ / (1024 * 1024)) << " MB");
}

// InferenceBuffers implementation

void InferenceBuffers::allocate(const ModelConfig& config,
                                 size_t batch_size, size_t max_seq_len) {
    size_t hidden_size = config.hidden_size;
    size_t num_layers = config.num_hidden_layers;
    size_t head_dim = config.head_dim();
    size_t num_kv_heads = config.num_key_value_heads;

    // Activation buffers
    size_t hidden_buf_size = batch_size * max_seq_len * hidden_size;
    hidden_states_ = CudaBuffer<__nv_bfloat16>(hidden_buf_size);
    residual_ = CudaBuffer<__nv_bfloat16>(hidden_buf_size);
    attn_output_ = CudaBuffer<__nv_bfloat16>(hidden_buf_size);

    // MLP buffer (intermediate size)
    size_t mlp_buf_size = batch_size * max_seq_len * config.intermediate_size;
    mlp_output_ = CudaBuffer<__nv_bfloat16>(mlp_buf_size);

    // Logits buffer (vocab size, float for precision)
    logits_ = CudaBuffer<float>(batch_size * config.vocab_size);

    // QKV buffers
    size_t qkv_size = batch_size * max_seq_len * hidden_size;
    q_ = CudaBuffer<__nv_bfloat16>(qkv_size);
    k_ = CudaBuffer<__nv_bfloat16>(batch_size * max_seq_len * num_kv_heads * head_dim);
    v_ = CudaBuffer<__nv_bfloat16>(batch_size * max_seq_len * num_kv_heads * head_dim);

    // KV cache per layer
    size_t kv_cache_size = batch_size * max_seq_len * num_kv_heads * head_dim;
    key_cache_.resize(num_layers);
    value_cache_.resize(num_layers);
    for (size_t i = 0; i < num_layers; ++i) {
        key_cache_[i] = CudaBuffer<__nv_bfloat16>(kv_cache_size);
        value_cache_[i] = CudaBuffer<__nv_bfloat16>(kv_cache_size);
    }

    LOG_INFO("Inference buffers allocated");
}

}  // namespace nemotron
