/**
 * @file ggml_model.h
 * @brief Internal ggml model structures for safetensors.cpp (Task 27)
 *
 * This file defines the ggml-based data structures for LLM inference,
 * supporting transformer architectures like Llama, Mistral, etc.
 */

#ifndef STCPP_GGML_MODEL_H
#define STCPP_GGML_MODEL_H

#include "safetensors.h"
#include "safetensors_internal.h"
#include <ggml.h>
#include <ggml-backend.h>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

namespace stcpp {

/* Model architecture types */
enum class ArchType {
    LLAMA,      // Llama, Llama2, Llama3
    MISTRAL,    // Mistral, Mixtral
    QWEN,       // Qwen, Qwen2
    PHI,        // Phi-2, Phi-3
    GEMMA,      // Gemma
    NEMOTRON,   // Nemotron-3 (Hybrid Mamba-Transformer MoE)
    GPT_OSS,    // gpt-oss
    GLM,        // GLM-4 MoE
    UNKNOWN
};

/* Model hyperparameters (from config.json) */
struct ModelHParams {
    int32_t n_vocab = 0;
    int32_t n_ctx_train = 0;   // Training context size
    int32_t n_embd = 0;        // Hidden size
    int32_t n_head = 0;        // Number of attention heads
    int32_t n_head_kv = 0;     // Number of KV heads (for GQA/MQA)
    int32_t n_layer = 0;       // Number of layers
    int32_t n_ff = 0;          // FFN intermediate size
    int32_t n_rot = 0;         // RoPE rotation dimensions
    float rope_freq_base = 10000.0f;
    float rope_freq_scale = 1.0f;
    float norm_eps = 1e-5f;
    bool use_gqa = false;      // Grouped Query Attention
    ArchType arch = ArchType::LLAMA;
    enum ggml_type weight_type = GGML_TYPE_F16;  // Weight data type (from torch_dtype)
};

/* Layer tensors for transformer layer */
struct LayerTensors {
    // Attention
    struct ggml_tensor* wq = nullptr;      // Query projection
    struct ggml_tensor* wk = nullptr;      // Key projection
    struct ggml_tensor* wv = nullptr;      // Value projection
    struct ggml_tensor* wo = nullptr;      // Output projection
    struct ggml_tensor* bq = nullptr;      // Query bias (optional, used by Qwen2)
    struct ggml_tensor* bk = nullptr;      // Key bias (optional, used by Qwen2)
    struct ggml_tensor* bv = nullptr;      // Value bias (optional, used by Qwen2)
    bool has_bq = false;                   // True if bq was loaded from model
    bool has_bk = false;                   // True if bk was loaded from model
    bool has_bv = false;                   // True if bv was loaded from model

    // Attention norm (pre-attention)
    struct ggml_tensor* attn_norm = nullptr;
    struct ggml_tensor* attn_norm_b = nullptr;  // Optional bias

    // FFN
    struct ggml_tensor* ffn_gate = nullptr;    // Gate projection (w1)
    struct ggml_tensor* ffn_up = nullptr;      // Up projection (w3)
    struct ggml_tensor* ffn_down = nullptr;    // Down projection (w2)

    // FFN norm (pre-FFN)
    struct ggml_tensor* ffn_norm = nullptr;
    struct ggml_tensor* ffn_norm_b = nullptr;  // Optional bias
};

/* Embedding and output tensors */
struct ModelTensors {
    // Token embedding
    struct ggml_tensor* tok_embd = nullptr;

    // Output
    struct ggml_tensor* output_norm = nullptr;
    struct ggml_tensor* output_norm_b = nullptr;
    struct ggml_tensor* output = nullptr;      // lm_head

    // Layers
    std::vector<LayerTensors> layers;
};

/* ggml model structure */
struct GgmlModel {
    ModelHParams hparams;
    ModelTensors tensors;

    // ggml contexts
    struct ggml_context* ctx_weights = nullptr;  // For model weights

    // Backend
    ggml_backend_t backend = nullptr;
    ggml_backend_buffer_t buffer = nullptr;

    // Model file info
    std::string model_path;
    std::vector<std::string> shard_paths;

    // Chat token support flag
    // If false, chat special tokens have identical embeddings (base model, not instruct)
    bool has_trained_chat_tokens = true;

    // Memory mapped files
    std::vector<void*> mmap_ptrs;
    std::vector<size_t> mmap_sizes;

    ~GgmlModel();
};

/* ggml inference context */
struct GgmlContext {
    GgmlModel* model = nullptr;
    stcpp_context_params params;

    // Compute context (recreated per batch)
    struct ggml_context* ctx_compute = nullptr;

    // KV cache
    struct ggml_tensor* k_cache = nullptr;
    struct ggml_tensor* v_cache = nullptr;
    ggml_backend_buffer_t kv_cache_buffer = nullptr;  // Backend buffer for KV cache
    int32_t kv_used = 0;     // Number of KV cache slots used
    int32_t kv_size = 0;     // Total KV cache size

    // State
    std::atomic<bool> cancel_flag{false};

    ~GgmlContext();
};

/* Tensor name mapping for different architectures */
struct TensorNameMap {
    // Common name patterns to ggml tensor pointers
    // Different models use different naming conventions
    static std::string normalize_name(const std::string& name, ArchType arch);
};

/* Model loading functions */

// Detect architecture from config.json
ArchType detect_architecture(const std::string& model_dir, std::string& error);

// Load model hyperparameters from config.json
bool load_hparams(
    const std::string& model_dir,
    ModelHParams& hparams,
    std::string& error
);

// Create ggml model from safetensors files
GgmlModel* load_ggml_model(
    const std::string& model_dir,
    stcpp_backend_type backend,
    int32_t device_id,
    std::string& error
);

// Create inference context
GgmlContext* create_ggml_context(
    GgmlModel* model,
    stcpp_context_params params,
    std::string& error
);

/* Compute graph functions */

// Build compute graph for forward pass
struct ggml_cgraph* build_compute_graph(
    GgmlContext* ctx,
    const int32_t* tokens,
    int32_t n_tokens,
    int32_t n_past
);

// Run forward pass and get logits
bool forward_pass(
    GgmlContext* ctx,
    const int32_t* tokens,
    int32_t n_tokens,
    int32_t n_past,
    float* logits,
    std::string& error
);

/* KV cache functions */

// Allocate KV cache
bool allocate_kv_cache(
    GgmlContext* ctx,
    int32_t n_ctx,
    std::string& error
);

// Clear KV cache
void clear_kv_cache(GgmlContext* ctx);

/* Utility functions */

// Get size needed for compute buffer
size_t estimate_compute_buffer_size(
    const ModelHParams& hparams,
    int32_t n_ctx,
    int32_t n_batch
);

// Convert safetensors dtype to ggml type
enum ggml_type dtype_to_ggml_type(DType dtype);

}  // namespace stcpp

#endif  // STCPP_GGML_MODEL_H
