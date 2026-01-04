#include <gpt-oss.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cctype>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_6.h>
#include <wrl/client.h>
#include <DirectML.h>
#endif

namespace fs = std::filesystem;

namespace {

#ifdef _WIN32
using Microsoft::WRL::ComPtr;

struct DmlRuntime {
    ComPtr<IDXGIAdapter1> adapter;
    ComPtr<ID3D12Device> device;
    ComPtr<IDMLDevice> dml_device;
    HMODULE dml_module{nullptr};
    bool initialized{false};
    std::string error;
};

bool init_dml_runtime(DmlRuntime& runtime) {
    ComPtr<IDXGIFactory6> factory;
    HRESULT hr = CreateDXGIFactory1(IID_PPV_ARGS(&factory));
    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create DXGI factory";
        return false;
    }

    ComPtr<IDXGIAdapter1> adapter;
    for (UINT index = 0; factory->EnumAdapterByGpuPreference(index, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                                                             IID_PPV_ARGS(&adapter)) != DXGI_ERROR_NOT_FOUND; ++index) {
        DXGI_ADAPTER_DESC1 desc = {};
        if (FAILED(adapter->GetDesc1(&desc))) continue;
        if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) continue;
        runtime.adapter = adapter;
        break;
    }

    if (!runtime.adapter) {
        runtime.error = "DirectML: no compatible GPU adapter found";
        return false;
    }

    hr = D3D12CreateDevice(runtime.adapter.Get(), D3D_FEATURE_LEVEL_12_0, IID_PPV_ARGS(&runtime.device));
    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create D3D12 device";
        return false;
    }

    runtime.dml_module = LoadLibraryW(L"DirectML.dll");
    if (!runtime.dml_module) {
        runtime.error = "DirectML: DirectML.dll not found";
        return false;
    }

    using DmlCreateDeviceFn =
        HRESULT(WINAPI*)(ID3D12Device*, DML_CREATE_DEVICE_FLAGS, REFIID, void**);
    using DmlCreateDevice1Fn =
        HRESULT(WINAPI*)(ID3D12Device*, DML_CREATE_DEVICE_FLAGS, DML_FEATURE_LEVEL, REFIID, void**);

    auto create1_fn = reinterpret_cast<DmlCreateDevice1Fn>(
        GetProcAddress(runtime.dml_module, "DMLCreateDevice1"));
    auto create_fn = reinterpret_cast<DmlCreateDeviceFn>(
        GetProcAddress(runtime.dml_module, "DMLCreateDevice"));
    if (create1_fn) {
        hr = create1_fn(runtime.device.Get(),
                        DML_CREATE_DEVICE_FLAG_NONE,
                        DML_FEATURE_LEVEL_1_0,
                        IID_PPV_ARGS(&runtime.dml_device));
        if (FAILED(hr) && create_fn) {
            hr = create_fn(runtime.device.Get(),
                           DML_CREATE_DEVICE_FLAG_NONE,
                           IID_PPV_ARGS(&runtime.dml_device));
        }
    } else if (create_fn) {
        hr = create_fn(runtime.device.Get(),
                       DML_CREATE_DEVICE_FLAG_NONE,
                       IID_PPV_ARGS(&runtime.dml_device));
    } else {
        runtime.error = "DirectML: DMLCreateDevice not exported";
        return false;
    }

    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create DML device";
        return false;
    }

    runtime.initialized = true;
    return true;
}

bool ensure_dml_runtime(std::string& error) {
    static DmlRuntime runtime;
    static std::once_flag once;
    std::call_once(once, [&]() { runtime.initialized = init_dml_runtime(runtime); });
    if (!runtime.initialized) {
        error = runtime.error;
        return false;
    }
    return true;
}
#endif

struct GptossTokenizer {
    std::atomic<uint32_t> ref_count{1};
    std::vector<std::string> id_to_token;
    std::unordered_map<std::string, uint32_t> token_to_id;
    std::array<uint32_t, gptoss_special_token_max - 1> special_ids{};
    uint32_t num_special_tokens{0};
};

struct GptossModel {
    std::atomic<uint32_t> ref_count{1};
    std::string model_dir;
    gptoss_tokenizer_t tokenizer{nullptr};
    uint32_t max_context_length{0};
};

struct GptossContext {
    std::atomic<uint32_t> ref_count{1};
    gptoss_model_t model{nullptr};
    std::vector<uint32_t> tokens;
    size_t max_tokens{0};
};

uint32_t find_unknown_token_id(const GptossTokenizer& tokenizer) {
    static const char* kUnknownTokens[] = {"<unk>", "<|unk|>", "<|unknown|>"};
    for (const char* candidate : kUnknownTokens) {
        auto it = tokenizer.token_to_id.find(candidate);
        if (it != tokenizer.token_to_id.end()) return it->second;
    }
    return UINT32_MAX;
}

bool parse_vocab(const nlohmann::json& model, GptossTokenizer& tokenizer) {
    if (!model.is_object()) return false;
    auto vocab_it = model.find("vocab");
    if (vocab_it == model.end() || !vocab_it->is_object()) return false;

    size_t max_id = 0;
    for (const auto& item : vocab_it->items()) {
        if (!item.value().is_number_unsigned()) continue;
        max_id = std::max(max_id, static_cast<size_t>(item.value().get<uint32_t>()));
    }

    tokenizer.id_to_token.assign(max_id + 1, "");
    for (const auto& item : vocab_it->items()) {
        if (!item.value().is_number_unsigned()) continue;
        const uint32_t id = item.value().get<uint32_t>();
        if (id >= tokenizer.id_to_token.size()) continue;
        tokenizer.id_to_token[id] = item.key();
        tokenizer.token_to_id[item.key()] = id;
    }

    return !tokenizer.id_to_token.empty();
}

void map_special_tokens(GptossTokenizer& tokenizer) {
    tokenizer.special_ids.fill(UINT32_MAX);
    const std::pair<const char*, gptoss_special_token> mapping[] = {
        {"<|return|>", gptoss_special_token_return},
        {"<|start|>", gptoss_special_token_start},
        {"<|message|>", gptoss_special_token_message},
        {"<|end|>", gptoss_special_token_end},
        {"<|refusal|>", gptoss_special_token_refusal},
        {"<|constrain|>", gptoss_special_token_constrain},
        {"<|channel|>", gptoss_special_token_channel},
        {"<|call|>", gptoss_special_token_call},
        {"<|untrusted|>", gptoss_special_token_untrusted},
        {"<|end_untrusted|>", gptoss_special_token_end_untrusted},
    };

    for (const auto& entry : mapping) {
        auto it = tokenizer.token_to_id.find(entry.first);
        if (it != tokenizer.token_to_id.end()) {
            tokenizer.special_ids[entry.second - 1] = it->second;
            tokenizer.num_special_tokens += 1;
        }
    }
}

bool load_tokenizer(const fs::path& tokenizer_path, GptossTokenizer& tokenizer) {
    std::ifstream in(tokenizer_path);
    if (!in) return false;

    nlohmann::json json;
    in >> json;

    const auto model_it = json.find("model");
    if (model_it == json.end() || !parse_vocab(*model_it, tokenizer)) return false;

    map_special_tokens(tokenizer);
    return true;
}

uint32_t load_max_context_length(const fs::path& config_path) {
    std::ifstream in(config_path);
    if (!in) return 0;

    nlohmann::json json;
    in >> json;
    for (const auto& key : {"max_position_embeddings", "context_length"}) {
        auto it = json.find(key);
        if (it != json.end() && it->is_number_unsigned()) {
            return it->get<uint32_t>();
        }
    }
    return 0;
}

gptoss_status tokenize_into(const GptossTokenizer& tokenizer, const char* text, size_t size, std::vector<uint32_t>& out) {
    const uint32_t unknown_id = find_unknown_token_id(tokenizer);
    std::string current;
    current.reserve(16);

    auto flush = [&]() -> gptoss_status {
        if (current.empty()) return gptoss_status_success;
        auto it = tokenizer.token_to_id.find(current);
        if (it != tokenizer.token_to_id.end()) {
            out.push_back(it->second);
        } else if (unknown_id != UINT32_MAX) {
            out.push_back(unknown_id);
        } else {
            return gptoss_status_invalid_argument;
        }
        current.clear();
        return gptoss_status_success;
    };

    for (size_t i = 0; i < size; i++) {
        const unsigned char ch = static_cast<unsigned char>(text[i]);
        if (std::isspace(ch)) {
            auto st = flush();
            if (st != gptoss_status_success) return st;
            std::string space(1, static_cast<char>(ch));
            auto it = tokenizer.token_to_id.find(space);
            if (it != tokenizer.token_to_id.end()) {
                out.push_back(it->second);
            } else if (unknown_id != UINT32_MAX) {
                out.push_back(unknown_id);
            } else {
                return gptoss_status_invalid_argument;
            }
        } else {
            current.push_back(static_cast<char>(ch));
        }
    }

    return flush();
}

}  // namespace

extern "C" {

gptoss_status GPTOSS_ABI gptoss_model_create_from_file(
    const char* model_path,
    gptoss_model_t* model_out) {
    if (!model_out) return gptoss_status_invalid_argument;
    *model_out = nullptr;
    if (!model_path || model_path[0] == '\0') return gptoss_status_invalid_argument;

    fs::path path(model_path);
    fs::path model_dir = fs::is_directory(path) ? path : path.parent_path();
    if (model_dir.empty()) return gptoss_status_invalid_argument;

#ifdef _WIN32
    std::string dml_error;
    if (!ensure_dml_runtime(dml_error)) {
        return gptoss_status_unsupported_system;
    }
#endif

    const fs::path config_path = model_dir / "config.json";
    const fs::path tokenizer_path = model_dir / "tokenizer.json";
    if (!fs::exists(config_path) || !fs::exists(tokenizer_path)) {
        return gptoss_status_io_error;
    }

    auto model = new (std::nothrow) GptossModel();
    if (!model) return gptoss_status_insufficient_memory;
    model->model_dir = model_dir.string();
    model->max_context_length = load_max_context_length(config_path);

    auto tokenizer = new (std::nothrow) GptossTokenizer();
    if (!tokenizer) {
        delete model;
        return gptoss_status_insufficient_memory;
    }
    if (!load_tokenizer(tokenizer_path, *tokenizer)) {
        delete tokenizer;
        delete model;
        return gptoss_status_io_error;
    }

    model->tokenizer = reinterpret_cast<gptoss_tokenizer_t>(tokenizer);
    *model_out = reinterpret_cast<gptoss_model_t>(model);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_get_tokenizer(
    gptoss_model_t model,
    gptoss_tokenizer_t* tokenizer_out) {
    if (!model || !tokenizer_out) return gptoss_status_invalid_argument;
    *tokenizer_out = reinterpret_cast<GptossModel*>(model)->tokenizer;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_get_max_context_length(
    gptoss_model_t model,
    uint32_t* max_context_length_out) {
    if (!model || !max_context_length_out) return gptoss_status_invalid_argument;
    *max_context_length_out = reinterpret_cast<GptossModel*>(model)->max_context_length;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_retain(gptoss_model_t model) {
    if (!model) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossModel*>(model)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_release(gptoss_model_t model) {
    if (!model) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossModel*>(model);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        if (ptr->tokenizer) {
            gptoss_tokenizer_release(ptr->tokenizer);
        }
        delete ptr;
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_special_token_id(
    gptoss_tokenizer_t tokenizer,
    enum gptoss_special_token token_type,
    uint32_t* token_id_out) {
    if (!tokenizer || !token_id_out) return gptoss_status_invalid_argument;
    if (token_type <= gptoss_special_token_invalid || token_type >= gptoss_special_token_max) {
        return gptoss_status_invalid_argument;
    }
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    const uint32_t id = ptr->special_ids[token_type - 1];
    if (id == UINT32_MAX) return gptoss_status_invalid_argument;
    *token_id_out = id;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_text_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_text_tokens_out) {
    if (!tokenizer || !num_text_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    *num_text_tokens_out = static_cast<uint32_t>(ptr->id_to_token.size() - ptr->num_special_tokens);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_special_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_special_tokens_out) {
    if (!tokenizer || !num_special_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    *num_special_tokens_out = ptr->num_special_tokens;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_tokens_out) {
    if (!tokenizer || !num_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    *num_tokens_out = static_cast<uint32_t>(ptr->id_to_token.size());
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_decode(
    gptoss_tokenizer_t tokenizer,
    uint32_t token,
    const char** token_ptr_out,
    size_t* token_size_out) {
    if (!tokenizer || !token_ptr_out || !token_size_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (token >= ptr->id_to_token.size()) return gptoss_status_invalid_argument;
    const std::string& tok = ptr->id_to_token[token];
    *token_ptr_out = tok.c_str();
    *token_size_out = tok.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_retain(gptoss_tokenizer_t tokenizer) {
    if (!tokenizer) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossTokenizer*>(tokenizer)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_release(gptoss_tokenizer_t tokenizer) {
    if (!tokenizer) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        delete ptr;
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_create(
    gptoss_model_t model,
    size_t context_length,
    size_t max_batch_tokens,
    gptoss_context_t* context_out) {
    if (!context_out || !model) return gptoss_status_invalid_argument;
    *context_out = nullptr;

    auto* model_ptr = reinterpret_cast<GptossModel*>(model);
    const size_t max_tokens = context_length == 0 ? model_ptr->max_context_length : context_length;
    if (max_tokens == 0) return gptoss_status_invalid_argument;
    if (max_batch_tokens != 0 && max_batch_tokens > max_tokens) {
        return gptoss_status_invalid_argument;
    }

    auto* ctx = new (std::nothrow) GptossContext();
    if (!ctx) return gptoss_status_insufficient_memory;
    ctx->model = model;
    ctx->max_tokens = max_tokens;
    gptoss_model_retain(model);
    *context_out = reinterpret_cast<gptoss_context_t>(ctx);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_num_tokens(
    gptoss_context_t context,
    size_t* num_tokens_out) {
    if (!context || !num_tokens_out) return gptoss_status_invalid_argument;
    *num_tokens_out = reinterpret_cast<GptossContext*>(context)->tokens.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_max_tokens(
    gptoss_context_t context,
    size_t* max_tokens_out) {
    if (!context || !max_tokens_out) return gptoss_status_invalid_argument;
    *max_tokens_out = reinterpret_cast<GptossContext*>(context)->max_tokens;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_tokens(
    gptoss_context_t context,
    uint32_t* tokens_out,
    size_t max_tokens,
    size_t* num_tokens_out) {
    if (!context || !num_tokens_out) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    *num_tokens_out = ctx->tokens.size();
    if (max_tokens < ctx->tokens.size()) return gptoss_status_insufficient_memory;
    if (tokens_out && !ctx->tokens.empty()) {
        std::copy(ctx->tokens.begin(), ctx->tokens.end(), tokens_out);
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_append_chars(
    gptoss_context_t context,
    const char* text,
    size_t text_size,
    size_t* num_tokens_out) {
    if (!context || !text) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    auto* model = reinterpret_cast<GptossModel*>(ctx->model);
    auto* tokenizer = reinterpret_cast<GptossTokenizer*>(model->tokenizer);

    std::vector<uint32_t> new_tokens;
    auto st = tokenize_into(*tokenizer, text, text_size, new_tokens);
    if (st != gptoss_status_success) return st;

    if (ctx->tokens.size() + new_tokens.size() > ctx->max_tokens) {
        return gptoss_status_context_overflow;
    }
    ctx->tokens.insert(ctx->tokens.end(), new_tokens.begin(), new_tokens.end());
    if (num_tokens_out) *num_tokens_out = new_tokens.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_append_tokens(
    gptoss_context_t context,
    size_t num_tokens,
    const uint32_t* tokens) {
    if (!context || (num_tokens != 0 && !tokens)) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    if (ctx->tokens.size() + num_tokens > ctx->max_tokens) {
        return gptoss_status_context_overflow;
    }
    ctx->tokens.insert(ctx->tokens.end(), tokens, tokens + num_tokens);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_reset(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossContext*>(context)->tokens.clear();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_process(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    return gptoss_status_unsupported_system;
}

gptoss_status GPTOSS_ABI gptoss_context_sample(
    gptoss_context_t context,
    float /*temperature*/,
    uint64_t /*rng_state*/,
    size_t /*num_tokens*/,
    uint32_t* /*tokens_out*/,
    size_t* /*num_tokens_out*/) {
    if (!context) return gptoss_status_invalid_argument;
    return gptoss_status_unsupported_system;
}

gptoss_status GPTOSS_ABI gptoss_context_retain(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossContext*>(context)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_release(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossContext*>(context);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        if (ptr->model) {
            gptoss_model_release(ptr->model);
        }
        delete ptr;
    }
    return gptoss_status_success;
}

}  // extern "C"
