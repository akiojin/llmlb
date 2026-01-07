/**
 * @file ggml_model.cpp
 * @brief ggml model loading and tensor allocation (Task 27)
 */

#include "ggml_model.h"
#include <ggml-cpu.h>
#ifdef STCPP_USE_METAL
#include <ggml-metal.h>
#endif
#ifdef STCPP_USE_CUDA
#include <ggml-cuda.h>
#endif
#ifdef STCPP_USE_ROCM
#include <ggml-hip.h>
#endif
#ifdef STCPP_USE_VULKAN
#include <ggml-vulkan.h>
#endif
#include <fstream>
#include <filesystem>
#include <cstring>
#include <algorithm>
#include <cmath>

#ifdef _WIN32
#include <windows.h>
#else
#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#endif

namespace stcpp {

/* GgmlModel destructor */
GgmlModel::~GgmlModel() {
    // Free backend buffer
    if (buffer) {
        ggml_backend_buffer_free(buffer);
        buffer = nullptr;
    }

    // Free backend
    if (backend) {
        ggml_backend_free(backend);
        backend = nullptr;
    }

    // Free ggml context
    if (ctx_weights) {
        ggml_free(ctx_weights);
        ctx_weights = nullptr;
    }

    // Unmap files
    for (size_t i = 0; i < mmap_ptrs.size(); ++i) {
        if (mmap_ptrs[i]) {
#ifdef _WIN32
            UnmapViewOfFile(mmap_ptrs[i]);
#else
            munmap(mmap_ptrs[i], mmap_sizes[i]);
#endif
        }
    }
    mmap_ptrs.clear();
    mmap_sizes.clear();
}

/* GgmlContext destructor */
GgmlContext::~GgmlContext() {
    // Free KV cache tensors are part of ctx_compute

    if (ctx_compute) {
        ggml_free(ctx_compute);
        ctx_compute = nullptr;
    }
}

/* Convert safetensors dtype to ggml type */
enum ggml_type dtype_to_ggml_type(DType dtype) {
    switch (dtype) {
        case DType::F16:  return GGML_TYPE_F16;
        case DType::BF16: return GGML_TYPE_BF16;
        case DType::F32:  return GGML_TYPE_F32;
        case DType::I8:   return GGML_TYPE_I8;
        case DType::I16:  return GGML_TYPE_I16;
        case DType::I32:  return GGML_TYPE_I32;
        default:          return GGML_TYPE_F32;
    }
}

/* Tensor name normalization */
std::string TensorNameMap::normalize_name(const std::string& name, ArchType arch) {
    // Different models use different naming conventions
    // This function normalizes them to a common format

    std::string normalized = name;

    // Remove common prefixes
    const char* prefixes[] = {
        "model.", "transformer.", "language_model.", "gpt_neox.", ""
    };

    for (const char* prefix : prefixes) {
        if (normalized.find(prefix) == 0) {
            normalized = normalized.substr(strlen(prefix));
            break;
        }
    }

    // Normalize layer numbering
    // e.g., "layers.0." -> "blk.0."
    size_t pos = normalized.find("layers.");
    if (pos != std::string::npos) {
        normalized.replace(pos, 7, "blk.");
    }

    // Handle architecture-specific mappings
    (void)arch;  // Reserved for future architecture-specific handling

    return normalized;
}

/* Detect architecture from config.json */
ArchType detect_architecture(const std::string& model_dir, std::string& error) {
    namespace fs = std::filesystem;

    fs::path config_path = fs::path(model_dir) / "config.json";
    if (!fs::exists(config_path)) {
        error = "config.json not found";
        return ArchType::UNKNOWN;
    }

    std::ifstream file(config_path);
    if (!file.is_open()) {
        error = "Failed to open config.json";
        return ArchType::UNKNOWN;
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());

    // Simple architecture detection from model_type field
    if (content.find("\"llama\"") != std::string::npos ||
        content.find("\"LlamaForCausalLM\"") != std::string::npos) {
        return ArchType::LLAMA;
    }
    if (content.find("\"mistral\"") != std::string::npos ||
        content.find("\"MistralForCausalLM\"") != std::string::npos) {
        return ArchType::MISTRAL;
    }
    if (content.find("\"qwen\"") != std::string::npos ||
        content.find("\"Qwen2ForCausalLM\"") != std::string::npos) {
        return ArchType::QWEN;
    }
    if (content.find("\"phi\"") != std::string::npos ||
        content.find("\"PhiForCausalLM\"") != std::string::npos) {
        return ArchType::PHI;
    }
    if (content.find("\"gemma\"") != std::string::npos ||
        content.find("\"GemmaForCausalLM\"") != std::string::npos) {
        return ArchType::GEMMA;
    }

    // Default to Llama-like architecture
    return ArchType::LLAMA;
}

/* Load hyperparameters from config.json */
bool load_hparams(
    const std::string& model_dir,
    ModelHParams& hparams,
    std::string& error
) {
    namespace fs = std::filesystem;

    fs::path config_path = fs::path(model_dir) / "config.json";
    std::ifstream file(config_path);
    if (!file.is_open()) {
        error = "Failed to open config.json";
        return false;
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());

    // Parse JSON to extract hyperparameters
    const char* p = content.data();
    const char* end = p + content.size();

    // Helper lambda for simple key-value extraction
    auto find_int_value = [&](const std::string& key) -> int32_t {
        std::string search = "\"" + key + "\"";
        size_t pos = content.find(search);
        if (pos == std::string::npos) return 0;

        pos = content.find(':', pos);
        if (pos == std::string::npos) return 0;

        pos++;
        while (pos < content.size() && (content[pos] == ' ' || content[pos] == '\t')) {
            pos++;
        }

        int32_t value = 0;
        bool negative = false;
        if (pos < content.size() && content[pos] == '-') {
            negative = true;
            pos++;
        }
        while (pos < content.size() && content[pos] >= '0' && content[pos] <= '9') {
            value = value * 10 + (content[pos] - '0');
            pos++;
        }
        return negative ? -value : value;
    };

    auto find_float_value = [&](const std::string& key) -> float {
        std::string search = "\"" + key + "\"";
        size_t pos = content.find(search);
        if (pos == std::string::npos) return 0.0f;

        pos = content.find(':', pos);
        if (pos == std::string::npos) return 0.0f;

        pos++;
        while (pos < content.size() && (content[pos] == ' ' || content[pos] == '\t')) {
            pos++;
        }

        std::string num_str;
        while (pos < content.size() &&
               (content[pos] == '-' || content[pos] == '.' ||
                content[pos] == 'e' || content[pos] == 'E' ||
                (content[pos] >= '0' && content[pos] <= '9'))) {
            num_str += content[pos];
            pos++;
        }
        return num_str.empty() ? 0.0f : std::stof(num_str);
    };

    // Extract parameters with fallback names
    hparams.n_vocab = find_int_value("vocab_size");

    hparams.n_embd = find_int_value("hidden_size");
    if (hparams.n_embd == 0) hparams.n_embd = find_int_value("n_embd");

    hparams.n_head = find_int_value("num_attention_heads");
    if (hparams.n_head == 0) hparams.n_head = find_int_value("n_head");

    hparams.n_head_kv = find_int_value("num_key_value_heads");
    if (hparams.n_head_kv == 0) hparams.n_head_kv = hparams.n_head;

    hparams.n_layer = find_int_value("num_hidden_layers");
    if (hparams.n_layer == 0) hparams.n_layer = find_int_value("n_layer");

    hparams.n_ff = find_int_value("intermediate_size");
    if (hparams.n_ff == 0) {
        // Default: 4 * hidden_size for most models
        hparams.n_ff = 4 * hparams.n_embd;
    }

    hparams.n_ctx_train = find_int_value("max_position_embeddings");
    if (hparams.n_ctx_train == 0) hparams.n_ctx_train = find_int_value("n_positions");
    if (hparams.n_ctx_train == 0) hparams.n_ctx_train = 4096;  // Default

    // RoPE parameters
    hparams.rope_freq_base = find_float_value("rope_theta");
    if (hparams.rope_freq_base == 0.0f) hparams.rope_freq_base = 10000.0f;

    // Calculate rotation dimensions
    hparams.n_rot = hparams.n_embd / hparams.n_head;

    // Normalization epsilon
    hparams.norm_eps = find_float_value("rms_norm_eps");
    if (hparams.norm_eps == 0.0f) {
        hparams.norm_eps = find_float_value("layer_norm_eps");
    }
    if (hparams.norm_eps == 0.0f) {
        hparams.norm_eps = 1e-5f;
    }

    // Check for GQA
    hparams.use_gqa = (hparams.n_head_kv != hparams.n_head);

    // Detect architecture
    hparams.arch = detect_architecture(model_dir, error);

    // Validate
    if (hparams.n_vocab == 0 || hparams.n_embd == 0 ||
        hparams.n_head == 0 || hparams.n_layer == 0) {
        error = "Invalid model configuration: missing required parameters";
        return false;
    }

    (void)p;
    (void)end;

    return true;
}

/* Create ggml backend */
static ggml_backend_t create_backend(
    stcpp_backend_type backend_type,
    int32_t device_id,
    std::string& error
) {
    ggml_backend_t backend = nullptr;

    switch (backend_type) {
#ifdef STCPP_USE_METAL
        case STCPP_BACKEND_METAL:
            backend = ggml_backend_metal_init();
            if (!backend) {
                error = "Failed to initialize Metal backend";
            }
            break;
#endif

#ifdef STCPP_USE_CUDA
        case STCPP_BACKEND_CUDA:
            backend = ggml_backend_cuda_init(device_id);
            if (!backend) {
                error = "Failed to initialize CUDA backend";
            }
            break;
#endif

#ifdef STCPP_USE_VULKAN
        case STCPP_BACKEND_VULKAN:
            backend = ggml_backend_vk_init(device_id);
            if (!backend) {
                error = "Failed to initialize Vulkan backend";
            }
            break;
#endif

        default:
            // CPU fallback
            backend = ggml_backend_cpu_init();
            if (!backend) {
                error = "Failed to initialize CPU backend";
            }
            break;
    }

    (void)device_id;  // May be unused if backends not compiled
    return backend;
}

/* Memory map a file */
static void* mmap_file(const std::string& path, size_t& size, std::string& error) {
#ifdef _WIN32
    HANDLE hFile = CreateFileA(path.c_str(), GENERIC_READ, FILE_SHARE_READ,
                               nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (hFile == INVALID_HANDLE_VALUE) {
        error = "Failed to open file: " + path;
        return nullptr;
    }

    LARGE_INTEGER fileSize;
    if (!GetFileSizeEx(hFile, &fileSize)) {
        CloseHandle(hFile);
        error = "Failed to get file size: " + path;
        return nullptr;
    }
    size = fileSize.QuadPart;

    HANDLE hMapping = CreateFileMappingA(hFile, nullptr, PAGE_READONLY, 0, 0, nullptr);
    if (!hMapping) {
        CloseHandle(hFile);
        error = "Failed to create file mapping: " + path;
        return nullptr;
    }

    void* ptr = MapViewOfFile(hMapping, FILE_MAP_READ, 0, 0, 0);
    CloseHandle(hMapping);
    CloseHandle(hFile);

    if (!ptr) {
        error = "Failed to map file: " + path;
        return nullptr;
    }

    return ptr;
#else
    int fd = open(path.c_str(), O_RDONLY);
    if (fd < 0) {
        error = "Failed to open file: " + path;
        return nullptr;
    }

    struct stat st;
    if (fstat(fd, &st) < 0) {
        close(fd);
        error = "Failed to stat file: " + path;
        return nullptr;
    }
    size = st.st_size;

    void* ptr = mmap(nullptr, size, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);

    if (ptr == MAP_FAILED) {
        error = "Failed to mmap file: " + path;
        return nullptr;
    }

    return ptr;
#endif
}

/* Create layer tensors */
static bool create_layer_tensors(
    struct ggml_context* ctx,
    LayerTensors& layer,
    const ModelHParams& hparams,
    int layer_idx
) {
    const int32_t n_embd = hparams.n_embd;
    const int32_t n_head = hparams.n_head;
    const int32_t n_head_kv = hparams.n_head_kv;
    const int32_t head_dim = n_embd / n_head;
    const int32_t n_ff = hparams.n_ff;

    char name[128];

    // Attention norm
    snprintf(name, sizeof(name), "blk.%d.attn_norm.weight", layer_idx);
    layer.attn_norm = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, n_embd);
    ggml_set_name(layer.attn_norm, name);

    // Q, K, V projections
    snprintf(name, sizeof(name), "blk.%d.attn_q.weight", layer_idx);
    layer.wq = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_embd, n_head * head_dim);
    ggml_set_name(layer.wq, name);

    snprintf(name, sizeof(name), "blk.%d.attn_k.weight", layer_idx);
    layer.wk = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_embd, n_head_kv * head_dim);
    ggml_set_name(layer.wk, name);

    snprintf(name, sizeof(name), "blk.%d.attn_v.weight", layer_idx);
    layer.wv = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_embd, n_head_kv * head_dim);
    ggml_set_name(layer.wv, name);

    // Output projection
    snprintf(name, sizeof(name), "blk.%d.attn_output.weight", layer_idx);
    layer.wo = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_head * head_dim, n_embd);
    ggml_set_name(layer.wo, name);

    // FFN norm
    snprintf(name, sizeof(name), "blk.%d.ffn_norm.weight", layer_idx);
    layer.ffn_norm = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, n_embd);
    ggml_set_name(layer.ffn_norm, name);

    // FFN layers (SwiGLU: gate, up, down)
    snprintf(name, sizeof(name), "blk.%d.ffn_gate.weight", layer_idx);
    layer.ffn_gate = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_embd, n_ff);
    ggml_set_name(layer.ffn_gate, name);

    snprintf(name, sizeof(name), "blk.%d.ffn_up.weight", layer_idx);
    layer.ffn_up = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_embd, n_ff);
    ggml_set_name(layer.ffn_up, name);

    snprintf(name, sizeof(name), "blk.%d.ffn_down.weight", layer_idx);
    layer.ffn_down = ggml_new_tensor_2d(ctx, GGML_TYPE_F16, n_ff, n_embd);
    ggml_set_name(layer.ffn_down, name);

    return true;
}

/* Estimate memory needed for model weights */
static size_t estimate_weight_memory(const ModelHParams& hparams) {
    size_t mem = 0;

    // Token embeddings: vocab_size * n_embd * F16
    mem += (size_t)hparams.n_vocab * hparams.n_embd * sizeof(ggml_fp16_t);

    // Per layer
    const int32_t n_embd = hparams.n_embd;
    const int32_t n_head = hparams.n_head;
    const int32_t n_head_kv = hparams.n_head_kv;
    const int32_t head_dim = n_embd / n_head;
    const int32_t n_ff = hparams.n_ff;

    for (int i = 0; i < hparams.n_layer; ++i) {
        // Attention norm
        mem += n_embd * sizeof(float);

        // Q, K, V, O
        mem += (size_t)n_embd * n_head * head_dim * sizeof(ggml_fp16_t);      // Q
        mem += (size_t)n_embd * n_head_kv * head_dim * sizeof(ggml_fp16_t);   // K
        mem += (size_t)n_embd * n_head_kv * head_dim * sizeof(ggml_fp16_t);   // V
        mem += (size_t)n_head * head_dim * n_embd * sizeof(ggml_fp16_t);      // O

        // FFN norm
        mem += n_embd * sizeof(float);

        // FFN
        mem += (size_t)n_embd * n_ff * sizeof(ggml_fp16_t);  // gate
        mem += (size_t)n_embd * n_ff * sizeof(ggml_fp16_t);  // up
        mem += (size_t)n_ff * n_embd * sizeof(ggml_fp16_t);  // down
    }

    // Output norm
    mem += n_embd * sizeof(float);

    // LM head (may be tied to embeddings)
    mem += (size_t)n_embd * hparams.n_vocab * sizeof(ggml_fp16_t);

    return mem;
}

/* Load ggml model from safetensors */
GgmlModel* load_ggml_model(
    const std::string& model_dir,
    stcpp_backend_type backend_type,
    int32_t device_id,
    std::string& error
) {
    namespace fs = std::filesystem;

    // Create model
    auto model = std::make_unique<GgmlModel>();
    model->model_path = model_dir;

    // Load hyperparameters
    if (!load_hparams(model_dir, model->hparams, error)) {
        return nullptr;
    }

    // Create backend
    model->backend = create_backend(backend_type, device_id, error);
    if (!model->backend) {
        return nullptr;
    }

    // Estimate memory needed
    size_t weight_mem = estimate_weight_memory(model->hparams);

    // Create ggml context for weights
    struct ggml_init_params ctx_params = {
        .mem_size = weight_mem + ggml_tensor_overhead() * 1024,  // Extra for metadata
        .mem_buffer = nullptr,
        .no_alloc = true,  // We'll use backend buffer
    };

    model->ctx_weights = ggml_init(ctx_params);
    if (!model->ctx_weights) {
        error = "Failed to create ggml context";
        return nullptr;
    }

    // Create tensors
    ModelTensors& tensors = model->tensors;
    const ModelHParams& hparams = model->hparams;

    // Token embeddings
    tensors.tok_embd = ggml_new_tensor_2d(
        model->ctx_weights, GGML_TYPE_F16,
        hparams.n_embd, hparams.n_vocab
    );
    ggml_set_name(tensors.tok_embd, "token_embd.weight");

    // Layer tensors
    tensors.layers.resize(hparams.n_layer);
    for (int i = 0; i < hparams.n_layer; ++i) {
        if (!create_layer_tensors(model->ctx_weights, tensors.layers[i], hparams, i)) {
            error = "Failed to create layer tensors";
            return nullptr;
        }
    }

    // Output norm
    tensors.output_norm = ggml_new_tensor_1d(
        model->ctx_weights, GGML_TYPE_F32, hparams.n_embd
    );
    ggml_set_name(tensors.output_norm, "output_norm.weight");

    // LM head
    tensors.output = ggml_new_tensor_2d(
        model->ctx_weights, GGML_TYPE_F16,
        hparams.n_embd, hparams.n_vocab
    );
    ggml_set_name(tensors.output, "output.weight");

    // Allocate backend buffer
    model->buffer = ggml_backend_alloc_ctx_tensors(model->ctx_weights, model->backend);
    if (!model->buffer) {
        error = "Failed to allocate backend buffer";
        return nullptr;
    }

    // Find and load safetensors files
    std::vector<std::string> safetensors_files;
    fs::path index_path = fs::path(model_dir) / "model.safetensors.index.json";

    if (fs::exists(index_path)) {
        // Sharded model
        std::unordered_map<std::string, std::string> tensor_to_shard;
        if (!parse_index_json(index_path.string(), safetensors_files, tensor_to_shard, error)) {
            return nullptr;
        }
        // Convert relative paths to absolute
        for (auto& f : safetensors_files) {
            f = (fs::path(model_dir) / f).string();
        }
    } else {
        // Single file model
        fs::path single_path = fs::path(model_dir) / "model.safetensors";
        if (!fs::exists(single_path)) {
            error = "No safetensors file found in " + model_dir;
            return nullptr;
        }
        safetensors_files.push_back(single_path.string());
    }

    model->shard_paths = safetensors_files;

    // Load tensor data from safetensors files
    // For MVP: we parse headers and memory-map files
    // Full implementation would copy data to GPU

    for (const auto& shard_path : safetensors_files) {
        SafetensorsHeader header;
        if (!parse_safetensors_header(shard_path, header, error)) {
            return nullptr;
        }
        model->mmap_sizes.push_back(0);  // Will be set by mmap

        size_t file_size = 0;
        void* file_data = mmap_file(shard_path, file_size, error);
        if (!file_data) {
            return nullptr;
        }
        model->mmap_ptrs.push_back(file_data);
        model->mmap_sizes.back() = file_size;

        // Map tensor data to ggml tensors
        const uint8_t* data_base = static_cast<const uint8_t*>(file_data) + header.data_offset;

        for (const auto& tensor_info : header.tensors) {
            // Find corresponding ggml tensor
            std::string norm_name = TensorNameMap::normalize_name(tensor_info.name, hparams.arch);

            struct ggml_tensor* ggml_tensor = nullptr;

            // Match by name pattern
            // This is simplified - full implementation would have comprehensive mapping
            if (norm_name.find("embed_tokens") != std::string::npos ||
                norm_name.find("tok_embd") != std::string::npos ||
                norm_name.find("wte") != std::string::npos) {
                ggml_tensor = tensors.tok_embd;
            } else if (norm_name.find("lm_head") != std::string::npos ||
                       norm_name.find("output.weight") != std::string::npos) {
                ggml_tensor = tensors.output;
            } else if (norm_name.find("norm") != std::string::npos &&
                       norm_name.find("blk") == std::string::npos) {
                ggml_tensor = tensors.output_norm;
            }
            // Layer tensors would be matched here...

            if (ggml_tensor) {
                // Copy data to backend buffer
                const void* src_data = data_base + tensor_info.data_offset;
                ggml_backend_tensor_set(ggml_tensor, src_data, 0, tensor_info.data_size);
            }
        }
    }

    return model.release();
}

/* Create inference context */
GgmlContext* create_ggml_context(
    GgmlModel* model,
    stcpp_context_params params,
    std::string& error
) {
    if (!model) {
        error = "Model is null";
        return nullptr;
    }

    auto ctx = std::make_unique<GgmlContext>();
    ctx->model = model;
    ctx->params = params;
    ctx->kv_size = params.n_ctx;

    // Allocate KV cache
    if (!allocate_kv_cache(ctx.get(), params.n_ctx, error)) {
        return nullptr;
    }

    return ctx.release();
}

/* Allocate KV cache */
bool allocate_kv_cache(
    GgmlContext* ctx,
    int32_t n_ctx,
    std::string& error
) {
    const ModelHParams& hparams = ctx->model->hparams;

    const int32_t n_layer = hparams.n_layer;
    const int32_t n_head_kv = hparams.n_head_kv;
    const int32_t head_dim = hparams.n_embd / hparams.n_head;

    // KV cache size: n_layer * n_ctx * n_head_kv * head_dim * 2 (K and V) * sizeof(fp16)
    size_t kv_cache_size = (size_t)n_layer * n_ctx * n_head_kv * head_dim * 2 * sizeof(ggml_fp16_t);

    // Add overhead for ggml tensors
    size_t compute_size = kv_cache_size + ggml_tensor_overhead() * 100;

    struct ggml_init_params cache_params = {
        .mem_size = compute_size,
        .mem_buffer = nullptr,
        .no_alloc = false,
    };

    ctx->ctx_compute = ggml_init(cache_params);
    if (!ctx->ctx_compute) {
        error = "Failed to allocate KV cache context";
        return false;
    }

    // Create KV cache tensors
    ctx->k_cache = ggml_new_tensor_4d(
        ctx->ctx_compute, GGML_TYPE_F16,
        head_dim, n_head_kv, n_ctx, n_layer
    );

    ctx->v_cache = ggml_new_tensor_4d(
        ctx->ctx_compute, GGML_TYPE_F16,
        head_dim, n_head_kv, n_ctx, n_layer
    );

    ctx->kv_size = n_ctx;
    ctx->kv_used = 0;

    return true;
}

/* Clear KV cache */
void clear_kv_cache(GgmlContext* ctx) {
    if (ctx) {
        ctx->kv_used = 0;
        // Optionally zero out the cache memory
    }
}

/* Estimate compute buffer size */
size_t estimate_compute_buffer_size(
    const ModelHParams& hparams,
    int32_t n_ctx,
    int32_t n_batch
) {
    // Rough estimate for compute buffer
    // Actual size depends on the compute graph

    const size_t n_embd = hparams.n_embd;
    const size_t n_ff = hparams.n_ff;

    // Intermediate tensors during forward pass
    size_t mem = 0;

    // Input embeddings
    mem += n_batch * n_embd * sizeof(float);

    // Per layer intermediates
    mem += 2 * n_batch * n_embd * sizeof(float);        // Residual
    mem += n_batch * n_ctx * hparams.n_head * sizeof(float);  // Attention scores
    mem += n_batch * n_ff * sizeof(float);              // FFN intermediate

    // Add margin
    mem = mem * 2 + 64 * 1024 * 1024;  // 64MB extra

    return mem;
}

}  // namespace stcpp
