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

static void bf16_to_f32_buffer(const uint16_t* src, float* dst, size_t n_elements) {
    for (size_t i = 0; i < n_elements; ++i) {
        uint32_t bits = static_cast<uint32_t>(src[i]) << 16;
        float val;
        memcpy(&val, &bits, sizeof(float));
        dst[i] = val;
    }
}

static void f16_to_f32_buffer(const ggml_fp16_t* src, float* dst, size_t n_elements) {
    ggml_fp16_to_fp32_row(src, dst, static_cast<int>(n_elements));
}

bool pack_mxfp4_blocks_to_ggml(
    const uint8_t* blocks,
    const uint8_t* scales,
    const std::vector<int64_t>& blocks_shape,
    const std::vector<int64_t>& scales_shape,
    int64_t row_offset,
    int64_t row_count,
    int64_t n_cols,
    std::vector<uint8_t>& out,
    std::string& error
) {
    if (!blocks || !scales) {
        error = "mxfp4 blocks/scales are null";
        return false;
    }
    if (blocks_shape.size() != 4 || scales_shape.size() != 3) {
        error = "mxfp4 blocks/scales shapes are invalid";
        return false;
    }

    const int64_t n_expert = blocks_shape[0];
    const int64_t n_rows_total = blocks_shape[1];
    const int64_t n_blocks = blocks_shape[2];
    const int64_t block_bytes = blocks_shape[3];

    if (block_bytes != 16) {
        error = "mxfp4 blocks last dimension must be 16";
        return false;
    }
    if (scales_shape[0] != n_expert || scales_shape[1] != n_rows_total || scales_shape[2] != n_blocks) {
        error = "mxfp4 scales shape mismatch";
        return false;
    }
    if (row_offset < 0 || row_count <= 0 || row_offset + row_count > n_rows_total) {
        error = "mxfp4 row range out of bounds";
        return false;
    }
    if (n_blocks * 32 != n_cols) {
        error = "mxfp4 blocks do not match expected column count";
        return false;
    }

    const size_t row_size = static_cast<size_t>(n_blocks) * 17;
    const size_t total_rows = static_cast<size_t>(n_expert) * static_cast<size_t>(row_count);
    out.assign(row_size * total_rows, 0);

    for (int64_t e = 0; e < n_expert; ++e) {
        for (int64_t r = 0; r < row_count; ++r) {
            const int64_t src_row = row_offset + r;
            const size_t dst_row_index = static_cast<size_t>(e) * row_count + static_cast<size_t>(r);
            uint8_t* dst = out.data() + dst_row_index * row_size;

            for (int64_t b = 0; b < n_blocks; ++b) {
                const size_t idx = static_cast<size_t>((e * n_rows_total + src_row) * n_blocks + b);
                const uint8_t scale = scales[idx];
                const uint8_t* block = blocks + idx * block_bytes;
                const size_t dst_off = static_cast<size_t>(b) * 17;

                dst[dst_off] = scale;
                memcpy(dst + dst_off + 1, block, 16);
            }
        }
    }

    return true;
}

/* Tensor name normalization */
std::string TensorNameMap::normalize_name(const std::string& name) {
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

    return normalized;
}

/* Detect architecture from config.json */
std::string detect_architecture(const std::string& model_dir, std::string& error) {
    namespace fs = std::filesystem;

    fs::path config_path = fs::path(model_dir) / "config.json";
    if (!fs::exists(config_path)) {
        error = "config.json not found";
        return "";
    }

    std::ifstream file(config_path);
    if (!file.is_open()) {
        error = "Failed to open config.json";
        return "";
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());

    // Extract model_type from config.json
    // Look for "model_type": "xxx" pattern
    size_t pos = content.find("\"model_type\"");
    if (pos != std::string::npos) {
        size_t colon = content.find(':', pos);
        if (colon != std::string::npos) {
            size_t quote1 = content.find('"', colon);
            if (quote1 != std::string::npos) {
                size_t quote2 = content.find('"', quote1 + 1);
                if (quote2 != std::string::npos) {
                    return content.substr(quote1 + 1, quote2 - quote1 - 1);
                }
            }
        }
    }

    // Fallback: try to detect from architectures field
    pos = content.find("\"architectures\"");
    if (pos != std::string::npos) {
        size_t bracket = content.find('[', pos);
        if (bracket != std::string::npos) {
            size_t quote1 = content.find('"', bracket);
            if (quote1 != std::string::npos) {
                size_t quote2 = content.find('"', quote1 + 1);
                if (quote2 != std::string::npos) {
                    std::string arch_class = content.substr(quote1 + 1, quote2 - quote1 - 1);
                    // Convert class name to model type (e.g., "LlamaForCausalLM" -> "llama")
                    if (arch_class.find("Llama") != std::string::npos) return "llama";
                    if (arch_class.find("Mistral") != std::string::npos) return "mistral";
                    if (arch_class.find("Qwen") != std::string::npos) return "qwen";
                    if (arch_class.find("Phi") != std::string::npos) return "phi";
                    if (arch_class.find("Gemma") != std::string::npos) return "gemma";
                    if (arch_class.find("Nemotron") != std::string::npos) return "nemotron";
                    if (arch_class.find("Glm") != std::string::npos) return "glm";
                    return arch_class;  // Return as-is if not recognized
                }
            }
        }
    }

    return "";  // Unknown architecture
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

    hparams.head_dim = find_int_value("head_dim");
    if (hparams.head_dim == 0 && hparams.n_head > 0) {
        hparams.head_dim = hparams.n_embd / hparams.n_head;
    }

    hparams.n_layer = find_int_value("num_hidden_layers");
    if (hparams.n_layer == 0) hparams.n_layer = find_int_value("n_layer");

    hparams.n_ff = find_int_value("intermediate_size");
    if (hparams.n_ff == 0) {
        // Default: 4 * hidden_size for most models
        hparams.n_ff = 4 * hparams.n_embd;
    }

    hparams.n_expert = find_int_value("num_local_experts");
    if (hparams.n_expert == 0) hparams.n_expert = find_int_value("num_experts");
    if (hparams.n_expert == 0) hparams.n_expert = find_int_value("n_expert");

    hparams.n_expert_used = find_int_value("num_experts_per_tok");
    if (hparams.n_expert_used == 0) hparams.n_expert_used = find_int_value("experts_per_token");
    if (hparams.n_expert_used == 0) hparams.n_expert_used = find_int_value("n_expert_used");

    hparams.swiglu_limit = find_float_value("swiglu_limit");
    if (hparams.swiglu_limit == 0.0f) hparams.swiglu_limit = 7.0f;

    hparams.n_ctx_train = find_int_value("max_position_embeddings");
    if (hparams.n_ctx_train == 0) hparams.n_ctx_train = find_int_value("n_positions");
    if (hparams.n_ctx_train == 0) hparams.n_ctx_train = 4096;  // Default

    // RoPE parameters
    hparams.rope_freq_base = find_float_value("rope_theta");
    if (hparams.rope_freq_base == 0.0f) hparams.rope_freq_base = 10000.0f;

    // Calculate rotation dimensions
    hparams.n_rot = hparams.head_dim > 0 ? hparams.head_dim : (hparams.n_embd / hparams.n_head);

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
    hparams.architecture = detect_architecture(model_dir, error);
    hparams.use_moe = (hparams.architecture == "gpt_oss" &&
                       hparams.n_expert > 0 &&
                       hparams.n_expert_used > 0);

    // Parse torch_dtype for weight data type
    auto find_string_value = [&](const std::string& key) -> std::string {
        std::string search = "\"" + key + "\"";
        size_t pos = content.find(search);
        if (pos == std::string::npos) return "";

        pos = content.find(':', pos);
        if (pos == std::string::npos) return "";

        pos = content.find('"', pos);
        if (pos == std::string::npos) return "";

        pos++;
        size_t end_pos = content.find('"', pos);
        if (end_pos == std::string::npos) return "";

        return content.substr(pos, end_pos - pos);
    };

    std::string torch_dtype = find_string_value("torch_dtype");
    if (torch_dtype == "bfloat16") {
        hparams.weight_type = GGML_TYPE_BF16;
    } else if (torch_dtype == "float16") {
        hparams.weight_type = GGML_TYPE_F16;
    } else if (torch_dtype == "float32") {
        hparams.weight_type = GGML_TYPE_F32;
    } else if (hparams.architecture == "gpt_oss") {
        hparams.weight_type = GGML_TYPE_BF16;
    } else {
        // Default to F16 for unknown types
        hparams.weight_type = GGML_TYPE_F16;
    }

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
    const int32_t head_dim = hparams.head_dim;
    const int32_t q_dim = n_head * head_dim;
    const int32_t kv_dim = n_head_kv * head_dim;
    const int32_t n_ff = hparams.n_ff;
    const enum ggml_type wtype = hparams.weight_type;

    char name[128];

    // Attention norm (always F32 for metal binary ops)
    snprintf(name, sizeof(name), "blk.%d.attn_norm.weight", layer_idx);
    layer.attn_norm = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, n_embd);
    ggml_set_name(layer.attn_norm, name);

    // Q, K, V projections (use weight_type from config)
    snprintf(name, sizeof(name), "blk.%d.attn_q.weight", layer_idx);
    layer.wq = ggml_new_tensor_2d(ctx, wtype, n_embd, q_dim);
    ggml_set_name(layer.wq, name);

    snprintf(name, sizeof(name), "blk.%d.attn_k.weight", layer_idx);
    layer.wk = ggml_new_tensor_2d(ctx, wtype, n_embd, kv_dim);
    ggml_set_name(layer.wk, name);

    snprintf(name, sizeof(name), "blk.%d.attn_v.weight", layer_idx);
    layer.wv = ggml_new_tensor_2d(ctx, wtype, n_embd, kv_dim);
    ggml_set_name(layer.wv, name);

    // Q, K, V biases (optional, used by Qwen2 - always F32)
    snprintf(name, sizeof(name), "blk.%d.attn_q.bias", layer_idx);
    layer.bq = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, q_dim);
    ggml_set_name(layer.bq, name);

    snprintf(name, sizeof(name), "blk.%d.attn_k.bias", layer_idx);
    layer.bk = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, kv_dim);
    ggml_set_name(layer.bk, name);

    snprintf(name, sizeof(name), "blk.%d.attn_v.bias", layer_idx);
    layer.bv = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, kv_dim);
    ggml_set_name(layer.bv, name);

    // Output projection
    snprintf(name, sizeof(name), "blk.%d.attn_output.weight", layer_idx);
    layer.wo = ggml_new_tensor_2d(ctx, wtype, q_dim, n_embd);
    ggml_set_name(layer.wo, name);

    // FFN norm (always F32 for metal binary ops)
    snprintf(name, sizeof(name), "blk.%d.ffn_norm.weight", layer_idx);
    layer.ffn_norm = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, n_embd);
    ggml_set_name(layer.ffn_norm, name);

    if (hparams.use_moe) {
        layer.is_moe = true;

        snprintf(name, sizeof(name), "blk.%d.moe_router.weight", layer_idx);
        layer.moe_router = ggml_new_tensor_2d(ctx, wtype, n_embd, hparams.n_expert);
        ggml_set_name(layer.moe_router, name);

        snprintf(name, sizeof(name), "blk.%d.moe_router.bias", layer_idx);
        layer.moe_router_bias = ggml_new_tensor_1d(ctx, GGML_TYPE_F32, hparams.n_expert);
        ggml_set_name(layer.moe_router_bias, name);

        snprintf(name, sizeof(name), "blk.%d.moe_gate_exps.weight", layer_idx);
        layer.moe_gate_exps = ggml_new_tensor_3d(ctx, GGML_TYPE_MXFP4, n_embd, n_ff, hparams.n_expert);
        ggml_set_name(layer.moe_gate_exps, name);

        snprintf(name, sizeof(name), "blk.%d.moe_up_exps.weight", layer_idx);
        layer.moe_up_exps = ggml_new_tensor_3d(ctx, GGML_TYPE_MXFP4, n_embd, n_ff, hparams.n_expert);
        ggml_set_name(layer.moe_up_exps, name);

        snprintf(name, sizeof(name), "blk.%d.moe_down_exps.weight", layer_idx);
        layer.moe_down_exps = ggml_new_tensor_3d(ctx, GGML_TYPE_MXFP4, n_ff, n_embd, hparams.n_expert);
        ggml_set_name(layer.moe_down_exps, name);

        snprintf(name, sizeof(name), "blk.%d.moe_gate_exps.bias", layer_idx);
        layer.moe_gate_bias = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, n_ff, hparams.n_expert);
        ggml_set_name(layer.moe_gate_bias, name);

        snprintf(name, sizeof(name), "blk.%d.moe_up_exps.bias", layer_idx);
        layer.moe_up_bias = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, n_ff, hparams.n_expert);
        ggml_set_name(layer.moe_up_bias, name);

        snprintf(name, sizeof(name), "blk.%d.moe_down_exps.bias", layer_idx);
        layer.moe_down_bias = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, n_embd, hparams.n_expert);
        ggml_set_name(layer.moe_down_bias, name);
    } else {
        // FFN layers (SwiGLU: gate, up, down) - use weight_type from config
        snprintf(name, sizeof(name), "blk.%d.ffn_gate.weight", layer_idx);
        layer.ffn_gate = ggml_new_tensor_2d(ctx, wtype, n_embd, n_ff);
        ggml_set_name(layer.ffn_gate, name);

        snprintf(name, sizeof(name), "blk.%d.ffn_up.weight", layer_idx);
        layer.ffn_up = ggml_new_tensor_2d(ctx, wtype, n_embd, n_ff);
        ggml_set_name(layer.ffn_up, name);

        snprintf(name, sizeof(name), "blk.%d.ffn_down.weight", layer_idx);
        layer.ffn_down = ggml_new_tensor_2d(ctx, wtype, n_ff, n_embd);
        ggml_set_name(layer.ffn_down, name);
    }

    return true;
}

/* Estimate memory needed for model weights */
static size_t estimate_weight_memory(const ModelHParams& hparams) {
    size_t mem = 0;

    // Get weight type size (2 bytes for F16/BF16, 4 bytes for F32)
    const size_t wtype_size = ggml_type_size(hparams.weight_type);

    // Token embeddings
    mem += (size_t)hparams.n_vocab * hparams.n_embd * wtype_size;

    // Per layer
    const int32_t n_embd = hparams.n_embd;
    const int32_t n_head = hparams.n_head;
    const int32_t n_head_kv = hparams.n_head_kv;
    const int32_t head_dim = hparams.head_dim;
    const int32_t q_dim = n_head * head_dim;
    const int32_t kv_dim = n_head_kv * head_dim;
    const int32_t n_ff = hparams.n_ff;

    for (int i = 0; i < hparams.n_layer; ++i) {
        // Attention norm (always F32)
        mem += n_embd * sizeof(float);

        // Q, K, V, O weights (use weight_type)
        mem += (size_t)n_embd * q_dim * wtype_size;   // Q
        mem += (size_t)n_embd * kv_dim * wtype_size;  // K
        mem += (size_t)n_embd * kv_dim * wtype_size;  // V
        mem += (size_t)q_dim * n_embd * wtype_size;   // O

        // Q, K, V biases (always F32, optional but allocated)
        mem += (size_t)q_dim * sizeof(float);   // bq
        mem += (size_t)kv_dim * sizeof(float);  // bk
        mem += (size_t)kv_dim * sizeof(float);  // bv


        // FFN norm (always F32)
        mem += n_embd * sizeof(float);

        if (hparams.use_moe) {
            const size_t row_size_gate = ggml_row_size(GGML_TYPE_MXFP4, n_embd);
            const size_t row_size_down = ggml_row_size(GGML_TYPE_MXFP4, n_ff);

            mem += row_size_gate * (size_t)n_ff * (size_t)hparams.n_expert;  // gate
            mem += row_size_gate * (size_t)n_ff * (size_t)hparams.n_expert;  // up
            mem += row_size_down * (size_t)n_embd * (size_t)hparams.n_expert;  // down

            mem += (size_t)n_embd * (size_t)hparams.n_expert * wtype_size;   // router
            mem += (size_t)hparams.n_expert * sizeof(float);                 // router bias
            mem += (size_t)n_ff * (size_t)hparams.n_expert * sizeof(float);  // gate bias
            mem += (size_t)n_ff * (size_t)hparams.n_expert * sizeof(float);  // up bias
            mem += (size_t)n_embd * (size_t)hparams.n_expert * sizeof(float); // down bias
        } else {
            // FFN (use weight_type)
            mem += (size_t)n_embd * n_ff * wtype_size;  // gate
            mem += (size_t)n_embd * n_ff * wtype_size;  // up
            mem += (size_t)n_ff * n_embd * wtype_size;  // down
        }
    }

    // Output norm (always F32)
    mem += n_embd * sizeof(float);

    // LM head (use weight_type)
    mem += (size_t)n_embd * hparams.n_vocab * wtype_size;

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

    fprintf(stderr, "[DEBUG] load_ggml_model: starting for %s\n", model_dir.c_str());
    fflush(stderr);

    // Create model
    auto model = std::make_unique<GgmlModel>();
    model->model_path = model_dir;

    fprintf(stderr, "[DEBUG] load_ggml_model: loading hparams\n");
    fflush(stderr);

    // Load hyperparameters
    if (!load_hparams(model_dir, model->hparams, error)) {
        fprintf(stderr, "[DEBUG] load_ggml_model: hparams failed: %s\n", error.c_str());
        fflush(stderr);
        return nullptr;
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: hparams loaded, weight_type=%d, n_layer=%d, n_embd=%d\n",
            model->hparams.weight_type, model->hparams.n_layer, model->hparams.n_embd);
    fflush(stderr);

    // CPU backend doesn't support bf16 operations, convert to f32
    if (backend_type == STCPP_BACKEND_CPU && model->hparams.weight_type == GGML_TYPE_BF16) {
        fprintf(stderr, "[DEBUG] load_ggml_model: CPU backend - converting bf16 to f32\n");
        fflush(stderr);
        model->hparams.weight_type = GGML_TYPE_F32;
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: creating backend (type=%d)\n", backend_type);
    fflush(stderr);

    // Create backend
    model->backend = create_backend(backend_type, device_id, error);
    if (!model->backend) {
        fprintf(stderr, "[DEBUG] load_ggml_model: backend failed: %s\n", error.c_str());
        fflush(stderr);
        return nullptr;
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: backend created\n");
    fflush(stderr);

    fprintf(stderr, "[DEBUG] load_ggml_model: estimating memory\n");
    fflush(stderr);

    // Estimate memory needed
    size_t weight_mem = estimate_weight_memory(model->hparams);

    fprintf(stderr, "[DEBUG] load_ggml_model: weight_mem=%zu, creating ggml context\n", weight_mem);
    fflush(stderr);

    // Create ggml context for weights
    struct ggml_init_params ctx_params = {
        .mem_size = weight_mem + ggml_tensor_overhead() * 1024,  // Extra for metadata
        .mem_buffer = nullptr,
        .no_alloc = true,  // We'll use backend buffer
    };

    model->ctx_weights = ggml_init(ctx_params);
    if (!model->ctx_weights) {
        error = "Failed to create ggml context";
        fprintf(stderr, "[DEBUG] load_ggml_model: ggml_init failed\n");
        fflush(stderr);
        return nullptr;
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: ggml context created, creating tensors\n");
    fflush(stderr);

    // Create tensors
    ModelTensors& tensors = model->tensors;
    const ModelHParams& hparams = model->hparams;
    const enum ggml_type wtype = hparams.weight_type;

    fprintf(stderr, "[DEBUG] load_ggml_model: creating tok_embd tensor (wtype=%d)\n", wtype);
    fflush(stderr);

    // Token embeddings (use weight_type from config)
    tensors.tok_embd = ggml_new_tensor_2d(
        model->ctx_weights, wtype,
        hparams.n_embd, hparams.n_vocab
    );
    ggml_set_name(tensors.tok_embd, "token_embd.weight");

    fprintf(stderr, "[DEBUG] load_ggml_model: creating layer tensors\n");
    fflush(stderr);

    // Layer tensors
    tensors.layers.resize(hparams.n_layer);
    for (int i = 0; i < hparams.n_layer; ++i) {
        if (!create_layer_tensors(model->ctx_weights, tensors.layers[i], hparams, i)) {
            error = "Failed to create layer tensors";
            fprintf(stderr, "[DEBUG] load_ggml_model: create_layer_tensors failed at layer %d\n", i);
            fflush(stderr);
            return nullptr;
        }
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: layer tensors created, creating output tensors\n");
    fflush(stderr);

    // Output norm (always F32 for metal binary ops)
    tensors.output_norm = ggml_new_tensor_1d(
        model->ctx_weights, GGML_TYPE_F32, hparams.n_embd
    );
    ggml_set_name(tensors.output_norm, "output_norm.weight");

    // LM head (use weight_type from config)
    tensors.output = ggml_new_tensor_2d(
        model->ctx_weights, wtype,
        hparams.n_embd, hparams.n_vocab
    );
    ggml_set_name(tensors.output, "output.weight");

    fprintf(stderr, "[DEBUG] load_ggml_model: allocating backend buffer\n");
    fflush(stderr);

    // Allocate backend buffer
    model->buffer = ggml_backend_alloc_ctx_tensors(model->ctx_weights, model->backend);
    if (!model->buffer) {
        error = "Failed to allocate backend buffer";
        fprintf(stderr, "[DEBUG] load_ggml_model: ggml_backend_alloc_ctx_tensors failed\n");
        fflush(stderr);
        return nullptr;
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: backend buffer allocated\n");
    fflush(stderr);

    fprintf(stderr, "[DEBUG] load_ggml_model: finding safetensors files\n");
    fflush(stderr);

    // Find and load safetensors files
    std::vector<std::string> safetensors_files;
    fs::path index_path = fs::path(model_dir) / "model.safetensors.index.json";

    if (fs::exists(index_path)) {
        // Sharded model
        fprintf(stderr, "[DEBUG] load_ggml_model: parsing index.json\n");
        fflush(stderr);
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
        fprintf(stderr, "[DEBUG] load_ggml_model: checking for %s\n", single_path.string().c_str());
        fflush(stderr);
        if (!fs::exists(single_path)) {
            error = "No safetensors file found in " + model_dir;
            return nullptr;
        }
        safetensors_files.push_back(single_path.string());
    }

    model->shard_paths = safetensors_files;
    fprintf(stderr, "[DEBUG] load_ggml_model: found %zu safetensors files\n", safetensors_files.size());
    fflush(stderr);

    struct Mxfp4Source {
        const uint8_t* blocks = nullptr;
        const uint8_t* scales = nullptr;
        std::vector<int64_t> blocks_shape;
        std::vector<int64_t> scales_shape;
    };

    struct GptOssMoeSources {
        Mxfp4Source gate_up;
        Mxfp4Source down;
        const uint8_t* gate_up_bias = nullptr;
        std::vector<int64_t> gate_up_bias_shape;
    };

    std::vector<GptOssMoeSources> gpt_oss_moe_sources;
    if (hparams.use_moe) {
        gpt_oss_moe_sources.resize(hparams.n_layer);
    }

    auto parse_layer_index = [](const std::string& norm_name, int& layer_idx) -> bool {
        size_t blk_pos = norm_name.find("blk.");
        if (blk_pos == std::string::npos) {
            return false;
        }
        size_t idx_start = blk_pos + 4;
        size_t idx_end = norm_name.find('.', idx_start);
        if (idx_end == std::string::npos) {
            return false;
        }
        layer_idx = std::stoi(norm_name.substr(idx_start, idx_end - idx_start));
        return true;
    };

    // Load tensor data from safetensors files
    // For MVP: we parse headers and memory-map files
    // Full implementation would copy data to GPU

    for (const auto& shard_path : safetensors_files) {
        fprintf(stderr, "[DEBUG] load_ggml_model: parsing header for %s\n", shard_path.c_str());
        fflush(stderr);

        SafetensorsHeader header;
        if (!parse_safetensors_header(shard_path, header, error)) {
            fprintf(stderr, "[DEBUG] load_ggml_model: parse_safetensors_header failed: %s\n", error.c_str());
            fflush(stderr);
            return nullptr;
        }
        model->mmap_sizes.push_back(0);  // Will be set by mmap

        fprintf(stderr, "[DEBUG] load_ggml_model: header parsed, %zu tensors, data_offset=%zu\n",
                header.tensors.size(), header.data_offset);
        fflush(stderr);

        size_t file_size = 0;
        void* file_data = mmap_file(shard_path, file_size, error);
        if (!file_data) {
            fprintf(stderr, "[DEBUG] load_ggml_model: mmap_file failed: %s\n", error.c_str());
            fflush(stderr);
            return nullptr;
        }
        model->mmap_ptrs.push_back(file_data);
        model->mmap_sizes.back() = file_size;

        fprintf(stderr, "[DEBUG] load_ggml_model: file mapped, size=%zu\n", file_size);
        fflush(stderr);

        // Map tensor data to ggml tensors
        const uint8_t* data_base = static_cast<const uint8_t*>(file_data) + header.data_offset;

        for (const auto& tensor_info : header.tensors) {
            // Find corresponding ggml tensor
            std::string norm_name = TensorNameMap::normalize_name(tensor_info.name);
            const uint8_t* src_data = data_base + tensor_info.data_offset;

            if (hparams.use_moe && norm_name.find("mlp.experts.") != std::string::npos) {
                int layer_idx = -1;
                if (!parse_layer_index(norm_name, layer_idx) ||
                    layer_idx < 0 || layer_idx >= hparams.n_layer) {
                    error = "Invalid layer index for gpt-oss MoE tensor: " + tensor_info.name;
                    return nullptr;
                }

                GptOssMoeSources& moe = gpt_oss_moe_sources[layer_idx];
                if (norm_name.find("mlp.experts.gate_up_proj_blocks") != std::string::npos) {
                    moe.gate_up.blocks = src_data;
                    moe.gate_up.blocks_shape = tensor_info.shape;
                    continue;
                }
                if (norm_name.find("mlp.experts.gate_up_proj_scales") != std::string::npos) {
                    moe.gate_up.scales = src_data;
                    moe.gate_up.scales_shape = tensor_info.shape;
                    continue;
                }
                if (norm_name.find("mlp.experts.gate_up_proj_bias") != std::string::npos) {
                    moe.gate_up_bias = src_data;
                    moe.gate_up_bias_shape = tensor_info.shape;
                    continue;
                }
                if (norm_name.find("mlp.experts.down_proj_blocks") != std::string::npos) {
                    moe.down.blocks = src_data;
                    moe.down.blocks_shape = tensor_info.shape;
                    continue;
                }
                if (norm_name.find("mlp.experts.down_proj_scales") != std::string::npos) {
                    moe.down.scales = src_data;
                    moe.down.scales_shape = tensor_info.shape;
                    continue;
                }
            }

            // Skip most bias tensors except QKV and gpt-oss MoE biases
            bool is_bias = (tensor_info.name.find(".bias") != std::string::npos ||
                            tensor_info.name.find("_bias") != std::string::npos);
            bool is_qkv_bias = (tensor_info.name.find("q_proj.bias") != std::string::npos ||
                                tensor_info.name.find("k_proj.bias") != std::string::npos ||
                                tensor_info.name.find("v_proj.bias") != std::string::npos ||
                                tensor_info.name.find("attn_q.bias") != std::string::npos ||
                                tensor_info.name.find("attn_k.bias") != std::string::npos ||
                                tensor_info.name.find("attn_v.bias") != std::string::npos);
            bool is_moe_bias = hparams.use_moe &&
                               (norm_name.find("mlp.router.bias") != std::string::npos ||
                                norm_name.find("mlp.experts.down_proj_bias") != std::string::npos);
            if (is_bias && !is_qkv_bias && !is_moe_bias) {
                continue;
            }

            struct ggml_tensor* ggml_tensor = nullptr;

            // Match by name pattern for global tensors
            if (norm_name.find("embed_tokens") != std::string::npos ||
                norm_name.find("tok_embd") != std::string::npos ||
                norm_name.find("wte") != std::string::npos) {
                ggml_tensor = tensors.tok_embd;
            } else if (norm_name.find("lm_head") != std::string::npos ||
                       (norm_name.find("output") != std::string::npos &&
                        norm_name.find("blk") == std::string::npos &&
                        norm_name.find("norm") == std::string::npos)) {
                ggml_tensor = tensors.output;
            } else if ((norm_name.find("norm") != std::string::npos ||
                        norm_name.find("ln_f") != std::string::npos) &&
                       norm_name.find("blk") == std::string::npos &&
                       norm_name.find("attn") == std::string::npos &&
                       norm_name.find("ffn") == std::string::npos) {
                ggml_tensor = tensors.output_norm;
            }

            // Match layer tensors: extract layer index from "blk.{i}." pattern
            if (!ggml_tensor && norm_name.find("blk.") != std::string::npos) {
                // Extract layer index
                int layer_idx = -1;
                if (parse_layer_index(norm_name, layer_idx)) {
                    if (layer_idx >= 0 && layer_idx < hparams.n_layer) {
                        LayerTensors& layer = tensors.layers[layer_idx];
                        size_t idx_start = norm_name.find("blk.") + 4;
                        size_t idx_end = norm_name.find('.', idx_start);
                        if (idx_end == std::string::npos) {
                            continue;
                        }
                        std::string layer_part = norm_name.substr(idx_end + 1);

                        if (hparams.use_moe) {
                            if (layer_part.find("mlp.router.weight") != std::string::npos) {
                                ggml_tensor = layer.moe_router;
                            } else if (layer_part.find("mlp.router.bias") != std::string::npos) {
                                ggml_tensor = layer.moe_router_bias;
                            } else if (layer_part.find("mlp.experts.down_proj_bias") != std::string::npos) {
                                ggml_tensor = layer.moe_down_bias;
                            }
                        }

                        if (!ggml_tensor) {
                            // Attention norm
                            if (layer_part.find("attn_norm") != std::string::npos ||
                                layer_part.find("input_layernorm") != std::string::npos) {
                                ggml_tensor = layer.attn_norm;
                            }
                            // Q projection (check bias first)
                            else if ((layer_part.find("q_proj.bias") != std::string::npos ||
                                      layer_part.find("attn_q.bias") != std::string::npos ||
                                      (layer_part.find("self_attn.q") != std::string::npos && layer_part.find(".bias") != std::string::npos))) {
                                ggml_tensor = layer.bq;
                                layer.has_bq = true;
                            }
                            else if (layer_part.find("attn_q") != std::string::npos ||
                                     layer_part.find("q_proj") != std::string::npos ||
                                     layer_part.find("self_attn.q") != std::string::npos) {
                                ggml_tensor = layer.wq;
                            }
                            // K projection (check bias first)
                            else if ((layer_part.find("k_proj.bias") != std::string::npos ||
                                      layer_part.find("attn_k.bias") != std::string::npos ||
                                      (layer_part.find("self_attn.k") != std::string::npos && layer_part.find(".bias") != std::string::npos))) {
                                ggml_tensor = layer.bk;
                                layer.has_bk = true;
                            }
                            else if (layer_part.find("attn_k") != std::string::npos ||
                                     layer_part.find("k_proj") != std::string::npos ||
                                     layer_part.find("self_attn.k") != std::string::npos) {
                                ggml_tensor = layer.wk;
                            }
                            // V projection (check bias first)
                            else if ((layer_part.find("v_proj.bias") != std::string::npos ||
                                      layer_part.find("attn_v.bias") != std::string::npos ||
                                      (layer_part.find("self_attn.v") != std::string::npos && layer_part.find(".bias") != std::string::npos))) {
                                ggml_tensor = layer.bv;
                                layer.has_bv = true;
                            }
                            else if (layer_part.find("attn_v") != std::string::npos ||
                                     layer_part.find("v_proj") != std::string::npos ||
                                     layer_part.find("self_attn.v") != std::string::npos) {
                                ggml_tensor = layer.wv;
                            }
                            // O projection
                            else if (layer_part.find("attn_output") != std::string::npos ||
                                     layer_part.find("o_proj") != std::string::npos ||
                                     layer_part.find("self_attn.o") != std::string::npos) {
                                ggml_tensor = layer.wo;
                            }
                            // FFN norm
                            else if (layer_part.find("ffn_norm") != std::string::npos ||
                                     layer_part.find("post_attention_layernorm") != std::string::npos) {
                                ggml_tensor = layer.ffn_norm;
                            }
                            // FFN gate (SwiGLU)
                            else if (layer_part.find("ffn_gate") != std::string::npos ||
                                     layer_part.find("gate_proj") != std::string::npos ||
                                     layer_part.find("mlp.gate") != std::string::npos) {
                                ggml_tensor = layer.ffn_gate;
                            }
                            // FFN up
                            else if (layer_part.find("ffn_up") != std::string::npos ||
                                     layer_part.find("up_proj") != std::string::npos ||
                                     layer_part.find("mlp.up") != std::string::npos) {
                                ggml_tensor = layer.ffn_up;
                            }
                            // FFN down
                            else if (layer_part.find("ffn_down") != std::string::npos ||
                                     layer_part.find("down_proj") != std::string::npos ||
                                     layer_part.find("mlp.down") != std::string::npos) {
                                ggml_tensor = layer.ffn_down;
                            }
                        }
                    }
                }
            }

            if (ggml_tensor) {
                // Validate size before copying
                size_t ggml_size = ggml_nbytes(ggml_tensor);

                if (ggml_size == tensor_info.data_size) {
                    // Direct copy - sizes match
                    fprintf(stderr, "[DEBUG] copying tensor %s, size=%zu\n", tensor_info.name.c_str(), tensor_info.data_size);
                    fflush(stderr);
                    ggml_backend_tensor_set(ggml_tensor, src_data, 0, tensor_info.data_size);
                } else if (ggml_tensor->type == GGML_TYPE_F32 && tensor_info.dtype == DType::BF16 &&
                           ggml_size == tensor_info.data_size * 2) {
                    // Convert bf16 to f32
                    size_t n_elements = ggml_nelements(ggml_tensor);
                    std::vector<float> f32_data(n_elements);
                    const uint16_t* bf16_src = reinterpret_cast<const uint16_t*>(src_data);

                    bf16_to_f32_buffer(bf16_src, f32_data.data(), n_elements);

                    fprintf(stderr, "[DEBUG] converting tensor %s bf16->f32, n_elements=%zu\n",
                            tensor_info.name.c_str(), n_elements);
                    fflush(stderr);
                    ggml_backend_tensor_set(ggml_tensor, f32_data.data(), 0, ggml_size);
                } else if (ggml_tensor->type == GGML_TYPE_F32 && tensor_info.dtype == DType::F16 &&
                           ggml_size == tensor_info.data_size * 2) {
                    // Convert f16 to f32
                    size_t n_elements = ggml_nelements(ggml_tensor);
                    std::vector<float> f32_data(n_elements);
                    const ggml_fp16_t* f16_src = reinterpret_cast<const ggml_fp16_t*>(src_data);

                    f16_to_f32_buffer(f16_src, f32_data.data(), n_elements);

                    fprintf(stderr, "[DEBUG] converting tensor %s f16->f32, n_elements=%zu\n",
                            tensor_info.name.c_str(), n_elements);
                    fflush(stderr);
                    ggml_backend_tensor_set(ggml_tensor, f32_data.data(), 0, ggml_size);
                } else {
                    fprintf(stderr, "[DEBUG] size mismatch for %s: ggml=%zu, safetensors=%zu\n",
                            tensor_info.name.c_str(), ggml_size, tensor_info.data_size);
                    fflush(stderr);
                }
            }
        }
    }

    if (hparams.use_moe) {
        for (int layer_idx = 0; layer_idx < hparams.n_layer; ++layer_idx) {
            const GptOssMoeSources& moe_src = gpt_oss_moe_sources[layer_idx];
            LayerTensors& layer = tensors.layers[layer_idx];

            if (!layer.moe_gate_exps || !layer.moe_up_exps || !layer.moe_down_exps) {
                continue;
            }

            if (!moe_src.gate_up.blocks || !moe_src.gate_up.scales ||
                !moe_src.down.blocks || !moe_src.down.scales) {
                error = "Missing gpt-oss mxfp4 tensors for layer " + std::to_string(layer_idx);
                return nullptr;
            }

            std::vector<uint8_t> packed;
            if (!pack_mxfp4_blocks_to_ggml(
                    moe_src.gate_up.blocks,
                    moe_src.gate_up.scales,
                    moe_src.gate_up.blocks_shape,
                    moe_src.gate_up.scales_shape,
                    0,
                    hparams.n_ff,
                    hparams.n_embd,
                    packed,
                    error)) {
                error = "gpt-oss gate packing failed for layer " + std::to_string(layer_idx) + ": " + error;
                return nullptr;
            }
            ggml_backend_tensor_set(layer.moe_gate_exps, packed.data(), 0, packed.size());

            if (!pack_mxfp4_blocks_to_ggml(
                    moe_src.gate_up.blocks,
                    moe_src.gate_up.scales,
                    moe_src.gate_up.blocks_shape,
                    moe_src.gate_up.scales_shape,
                    hparams.n_ff,
                    hparams.n_ff,
                    hparams.n_embd,
                    packed,
                    error)) {
                error = "gpt-oss up packing failed for layer " + std::to_string(layer_idx) + ": " + error;
                return nullptr;
            }
            ggml_backend_tensor_set(layer.moe_up_exps, packed.data(), 0, packed.size());

            if (!pack_mxfp4_blocks_to_ggml(
                    moe_src.down.blocks,
                    moe_src.down.scales,
                    moe_src.down.blocks_shape,
                    moe_src.down.scales_shape,
                    0,
                    hparams.n_embd,
                    hparams.n_ff,
                    packed,
                    error)) {
                error = "gpt-oss down packing failed for layer " + std::to_string(layer_idx) + ": " + error;
                return nullptr;
            }
            ggml_backend_tensor_set(layer.moe_down_exps, packed.data(), 0, packed.size());

            if (moe_src.gate_up_bias) {
                const int64_t n_expert = hparams.n_expert;
                const int64_t n_ff = hparams.n_ff;
                if (moe_src.gate_up_bias_shape.size() != 2 ||
                    moe_src.gate_up_bias_shape[0] != n_expert ||
                    moe_src.gate_up_bias_shape[1] != n_ff * 2) {
                    error = "gpt-oss gate_up bias shape mismatch for layer " + std::to_string(layer_idx);
                    return nullptr;
                }

                std::vector<float> gate_bias(n_expert * n_ff);
                std::vector<float> up_bias(n_expert * n_ff);
                const uint16_t* src = reinterpret_cast<const uint16_t*>(moe_src.gate_up_bias);

                for (int64_t e = 0; e < n_expert; ++e) {
                    const uint16_t* row = src + e * n_ff * 2;
                    bf16_to_f32_buffer(row, gate_bias.data() + e * n_ff, static_cast<size_t>(n_ff));
                    bf16_to_f32_buffer(row + n_ff, up_bias.data() + e * n_ff, static_cast<size_t>(n_ff));
                }

                ggml_backend_tensor_set(layer.moe_gate_bias, gate_bias.data(), 0, gate_bias.size() * sizeof(float));
                ggml_backend_tensor_set(layer.moe_up_bias, up_bias.data(), 0, up_bias.size() * sizeof(float));
            }
        }
    }

    fprintf(stderr, "[DEBUG] load_ggml_model: all tensors loaded successfully\n");
    fflush(stderr);

    // Debug: verify tok_embd has non-zero values
    if (model->tensors.tok_embd && model->tensors.tok_embd->buffer) {
        std::vector<float> test_data(5);
        ggml_backend_tensor_get(model->tensors.tok_embd, test_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] load_ggml_model: tok_embd first 5 values: %.6f %.6f %.6f %.6f %.6f\n",
                test_data[0], test_data[1], test_data[2], test_data[3], test_data[4]);
        fflush(stderr);
    }

    // Check if output (lm_head) was loaded - if not, use tied embeddings
    if (model->tensors.output && model->tensors.output->buffer) {
        std::vector<float> test_data(5);
        ggml_backend_tensor_get(model->tensors.output, test_data.data(), 0, 5 * sizeof(float));

        // Check if output is all zeros (lm_head not found in safetensors)
        bool all_zeros = true;
        for (int i = 0; i < 5; ++i) {
            if (test_data[i] != 0.0f) {
                all_zeros = false;
                break;
            }
        }

        if (all_zeros && model->tensors.tok_embd && model->tensors.tok_embd->buffer) {
            // Tied embeddings: copy tok_embd data to output (lm_head)
            size_t tensor_size = ggml_nbytes(model->tensors.output);
            std::vector<char> buffer(tensor_size);
            ggml_backend_tensor_get(model->tensors.tok_embd, buffer.data(), 0, tensor_size);
            ggml_backend_tensor_set(model->tensors.output, buffer.data(), 0, tensor_size);

            fprintf(stderr, "[DEBUG] load_ggml_model: tied embeddings - copied tok_embd to output (lm_head), size=%zu\n",
                    tensor_size);
            fflush(stderr);

            // Verify the copy
            ggml_backend_tensor_get(model->tensors.output, test_data.data(), 0, 5 * sizeof(float));
        }

        fprintf(stderr, "[DEBUG] load_ggml_model: output (lm_head) first 5 values: %.6f %.6f %.6f %.6f %.6f\n",
                test_data[0], test_data[1], test_data[2], test_data[3], test_data[4]);
        fflush(stderr);
    }

    // Check if chat special tokens have distinct embeddings
    // If tokens 151644 (<|im_start|>) and 151645 (<|im_end|>) have identical embeddings,
    // this is likely a base model without properly trained chat tokens
    if (model->tensors.tok_embd && model->tensors.tok_embd->buffer) {
        const size_t n_embd = model->hparams.n_embd;
        const size_t emb_byte_size = n_embd * sizeof(float);

        // Check if vocab is large enough for chat tokens
        if (model->hparams.n_vocab >= 151646) {
            std::vector<float> emb_im_start(n_embd);
            std::vector<float> emb_im_end(n_embd);

            // Token 151644 = <|im_start|>, Token 151645 = <|im_end|>
            size_t offset_im_start = 151644 * model->tensors.tok_embd->nb[1];
            size_t offset_im_end = 151645 * model->tensors.tok_embd->nb[1];

            ggml_backend_tensor_get(model->tensors.tok_embd, emb_im_start.data(), offset_im_start, emb_byte_size);
            ggml_backend_tensor_get(model->tensors.tok_embd, emb_im_end.data(), offset_im_end, emb_byte_size);

            // Compare embeddings using epsilon for floating point comparison
            // BF16 -> F32 conversion can introduce tiny differences (~1e-5)
            // If max absolute difference is very small, consider them identical
            const float epsilon = 1e-4f;  // Tolerance for base model detection
            float max_diff = 0.0f;
            for (size_t i = 0; i < n_embd; ++i) {
                float diff = std::fabs(emb_im_start[i] - emb_im_end[i]);
                if (diff > max_diff) {
                    max_diff = diff;
                }
            }

            bool identical = (max_diff < epsilon);

            if (identical) {
                model->has_trained_chat_tokens = false;
                fprintf(stderr, "[WARNING] load_ggml_model: Chat tokens <|im_start|> and <|im_end|> have identical embeddings.\n");
                fprintf(stderr, "[WARNING] This appears to be a BASE model, not an INSTRUCT model.\n");
                fprintf(stderr, "[WARNING] Chat completions may produce garbage output.\n");
                fprintf(stderr, "[WARNING] Consider using the Instruct version (e.g., Qwen2.5-0.5B-Instruct).\n");
                fflush(stderr);
            } else {
                fprintf(stderr, "[DEBUG] load_ggml_model: Chat tokens have distinct embeddings (Instruct model)\n");
                fflush(stderr);
            }
        }
    }

    // Debug: verify tok_embd has non-zero values
    if (model->tensors.tok_embd && model->tensors.tok_embd->buffer) {
        std::vector<float> test_data(5);
        ggml_backend_tensor_get(model->tensors.tok_embd, test_data.data(), 0, 5 * sizeof(float));
        fprintf(stderr, "[DEBUG] load_ggml_model: tok_embd first 5 values: %.6f %.6f %.6f %.6f %.6f\n",
                test_data[0], test_data[1], test_data[2], test_data[3], test_data[4]);
        fflush(stderr);
    }

    // Check if output (lm_head) was loaded - if not, use tied embeddings
    if (model->tensors.output && model->tensors.output->buffer) {
        std::vector<float> test_data(5);
        ggml_backend_tensor_get(model->tensors.output, test_data.data(), 0, 5 * sizeof(float));

        // Check if output is all zeros (lm_head not found in safetensors)
        bool all_zeros = true;
        for (int i = 0; i < 5; ++i) {
            if (test_data[i] != 0.0f) {
                all_zeros = false;
                break;
            }
        }

        if (all_zeros && model->tensors.tok_embd && model->tensors.tok_embd->buffer) {
            // Tied embeddings: copy tok_embd data to output (lm_head)
            size_t tensor_size = ggml_nbytes(model->tensors.output);
            std::vector<char> buffer(tensor_size);
            ggml_backend_tensor_get(model->tensors.tok_embd, buffer.data(), 0, tensor_size);
            ggml_backend_tensor_set(model->tensors.output, buffer.data(), 0, tensor_size);

            fprintf(stderr, "[DEBUG] load_ggml_model: tied embeddings - copied tok_embd to output (lm_head), size=%zu\n",
                    tensor_size);
            fflush(stderr);

            // Verify the copy
            ggml_backend_tensor_get(model->tensors.output, test_data.data(), 0, 5 * sizeof(float));
        }

        fprintf(stderr, "[DEBUG] load_ggml_model: output (lm_head) first 5 values: %.6f %.6f %.6f %.6f %.6f\n",
                test_data[0], test_data[1], test_data[2], test_data[3], test_data[4]);
        fflush(stderr);
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
    const int32_t head_dim = hparams.head_dim;

    // KV cache size: n_layer * n_ctx * n_head_kv * head_dim * 2 (K and V) * sizeof(fp16)
    size_t kv_cache_size = (size_t)n_layer * n_ctx * n_head_kv * head_dim * 2 * sizeof(ggml_fp16_t);

    // Add overhead for ggml tensors
    size_t compute_size = kv_cache_size + ggml_tensor_overhead() * 100;

    struct ggml_init_params cache_params = {
        .mem_size = compute_size,
        .mem_buffer = nullptr,
        .no_alloc = true,  // allocate via backend buffer
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

    if (!ctx->model || !ctx->model->backend) {
        error = "KV cache backend not initialized";
        return false;
    }

    ctx->kv_cache_buffer = ggml_backend_alloc_ctx_tensors(ctx->ctx_compute, ctx->model->backend);
    if (!ctx->kv_cache_buffer) {
        error = "Failed to allocate KV cache backend buffer";
        return false;
    }

    ggml_backend_buffer_clear(ctx->kv_cache_buffer, 0);

    ctx->kv_size = n_ctx;
    ctx->kv_used = 0;

    return true;
}

/* Clear KV cache */
void clear_kv_cache(GgmlContext* ctx) {
    if (ctx) {
        ctx->kv_used = 0;
        // Zero out the cache memory to prevent stale data issues
        if (ctx->kv_cache_buffer) {
            ggml_backend_buffer_clear(ctx->kv_cache_buffer, 0);
        }
    }
}

/* Estimate compute buffer size */
size_t estimate_compute_buffer_size(
    const ModelHParams& hparams,
    int32_t n_ctx,
    int32_t n_batch
) {
    // Rough estimate for compute buffer
    // Need to account for all intermediate tensors + graph overhead

    const size_t n_embd = hparams.n_embd;
    const size_t n_ff = hparams.n_ff;
    const size_t n_layer = hparams.n_layer;
    const size_t n_head = hparams.n_head;

    size_t mem = 0;

    // Input embeddings
    mem += n_batch * n_embd * sizeof(float);

    // Per layer costs (accumulated across all layers)
    size_t per_layer = 0;
    per_layer += 4 * n_batch * n_embd * sizeof(float);  // Residual, Q, K, V projections
    per_layer += n_batch * n_ctx * n_head * sizeof(float);  // Attention scores [n_ctx, n_batch, n_head]
    per_layer += n_batch * n_embd * sizeof(float);  // Attention output
    per_layer += 3 * n_batch * n_ff * sizeof(float);  // FFN gate, up, down intermediates
    per_layer += 2 * n_batch * n_embd * sizeof(float);  // RMSNorm intermediates
    if (hparams.use_moe) {
        per_layer += n_batch * hparams.n_expert * sizeof(float);  // Router logits
        per_layer += 4 * n_batch * hparams.n_expert_used * sizeof(float);  // Top-K indices/weights
    }

    mem += per_layer * n_layer;

    // Graph overhead (ggml_tensor objects, view tensors, permute results)
    // Each layer creates many intermediate tensors with overhead
    const size_t tensor_overhead = 512;  // bytes per tensor object
    const size_t tensors_per_layer = 30;  // approximate number of tensors per layer
    mem += n_layer * tensors_per_layer * tensor_overhead;

    // Extra margin for safety (2x multiplier + 128MB base)
    mem = mem * 2 + 128 * 1024 * 1024;

    return mem;
}

}  // namespace stcpp
