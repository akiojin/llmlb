#include "core/nemotron_engine.h"

#include <algorithm>
#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <functional>
#include <fstream>
#include <optional>
#include <random>
#include <sstream>
#include <stdexcept>

#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>
#include "utils/stop_sequences.h"

#ifdef _WIN32
#include <windows.h>
#endif

#ifdef USE_CUDA
#include <cuda_runtime.h>
#endif

#ifdef USE_GPTOSS
extern "C" {
#include <gpt-oss/functions.h>
#include <gpt-oss/types.h>
}
#endif

#define SAFETENSORS_CPP_IMPLEMENTATION
#include "safetensors.hh"

namespace fs = std::filesystem;

namespace llm_node {

namespace {
constexpr const char* kKnownTensorName = "backbone.layers.1.mixer.experts.0.down_proj.weight";
constexpr size_t kDefaultUploadMaxBytes = 64 * 1024 * 1024;
static const std::vector<std::string> kDefaultStopSequences = {
    "<|im_end|>",
    "<|end|>",
    "<|start|>",
    "<|eot_id|>",
    "</s>",
    "<|endoftext|>",
};

struct NemotronTokenEmitState {
    std::string output;
    std::optional<StopSequenceStream> stop_stream;
    bool stopped{false};
};

uint64_t steady_now_ns() {
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
}

void emit_token_metrics(const InferenceParams& params, uint32_t token_id) {
    if (!params.on_token_callback) return;
    params.on_token_callback(params.on_token_callback_ctx, token_id, steady_now_ns());
}

void emit_text_token(uint32_t token,
                     uint32_t num_text_tokens,
                     const std::function<std::string(uint32_t)>& decode,
                     NemotronTokenEmitState& state,
                     std::vector<std::string>* emitted,
                     const std::function<void(const std::string&)>& on_token) {
    if (num_text_tokens == 0 || token >= num_text_tokens) return;
    std::string piece = decode(token);
    if (piece.empty()) return;
    auto emit = [&](const std::string& chunk) {
        if (chunk.empty()) return;
        state.output.append(chunk);
        if (emitted) {
            emitted->push_back(chunk);
        }
        if (on_token) {
            on_token(chunk);
        }
    };
    if (state.stop_stream) {
        if (state.stop_stream->push(piece, emit)) {
            state.stopped = true;
        }
        return;
    }
    emit(piece);
}

bool is_regular_nonempty_file(const fs::path& path) {
    std::error_code ec;
    auto st = fs::symlink_status(path, ec);
    if (ec) return false;
    if (st.type() != fs::file_type::regular && st.type() != fs::file_type::symlink) return false;
    auto size = fs::file_size(path, ec);
    return !ec && size > 0;
}

#ifdef USE_CUDA
struct UploadedTensor {
    void* device_ptr{nullptr};
    size_t bytes{0};
};

std::optional<size_t> parse_env_size_bytes(const char* value) {
    if (!value) return std::nullopt;
    try {
        size_t parsed = static_cast<size_t>(std::stoull(value));
        return parsed;
    } catch (...) {
        return std::nullopt;
    }
}

std::optional<UploadedTensor> upload_tensor_to_gpu(const fs::path& path,
                                                   const std::string& tensor_name,
                                                   size_t max_bytes,
                                                   std::string& err) {
    safetensors::safetensors_t st;
    std::string warn;
    if (!safetensors::mmap_from_file(path.string(), &st, &warn, &err)) {
        err = err.empty() ? "Failed to mmap safetensors file" : err;
        return std::nullopt;
    }
    if (!warn.empty()) {
        spdlog::warn("NemotronEngine: safetensors warning: {}", warn);
    }
    safetensors::tensor_t tensor;
    if (!st.tensors.at(tensor_name, &tensor)) {
        err = "Tensor not found in safetensors: " + tensor_name;
        return std::nullopt;
    }
    if (tensor.data_offsets[1] <= tensor.data_offsets[0]) {
        err = "Invalid tensor data offsets for: " + tensor_name;
        return std::nullopt;
    }
    const size_t bytes = tensor.data_offsets[1] - tensor.data_offsets[0];
    if (bytes > max_bytes) {
        err = "Tensor size exceeds upload limit: " + std::to_string(bytes);
        return std::nullopt;
    }
    const uint8_t* src = st.databuffer_addr + tensor.data_offsets[0];
    void* device_ptr = nullptr;
    const cudaError_t alloc_status = cudaMalloc(&device_ptr, bytes);
    if (alloc_status != cudaSuccess || device_ptr == nullptr) {
        err = "cudaMalloc failed: " + std::string(cudaGetErrorString(alloc_status));
        return std::nullopt;
    }
    const cudaError_t copy_status = cudaMemcpy(device_ptr, src, bytes, cudaMemcpyHostToDevice);
    if (copy_status != cudaSuccess) {
        cudaFree(device_ptr);
        err = "cudaMemcpy failed: " + std::string(cudaGetErrorString(copy_status));
        return std::nullopt;
    }
    UploadedTensor uploaded;
    uploaded.device_ptr = device_ptr;
    uploaded.bytes = bytes;
    return uploaded;
}
#endif

#ifdef USE_GPTOSS
uint64_t resolve_seed(uint32_t seed) {
    if (seed != 0) return seed;
    const uint64_t t = static_cast<uint64_t>(
        std::chrono::steady_clock::now().time_since_epoch().count());
    uint64_t x = t + UINT64_C(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)) * UINT64_C(0x94d049bb133111eb);
    return x ^ (x >> 31);
}

fs::path resolve_nemotron_directml_model_bin(const fs::path& model_dir) {
    const fs::path p1 = model_dir / "model.directml.bin";
    if (fs::exists(p1)) return p1;
    const fs::path p2 = model_dir / "model.dml.bin";
    if (fs::exists(p2)) return p2;
    return {};
}

fs::path resolve_nemotron_directml_model_file(const ModelDescriptor& descriptor) {
    fs::path model_dir = descriptor.model_dir.empty()
                             ? fs::path(descriptor.primary_path).parent_path()
                             : fs::path(descriptor.model_dir);
    if (model_dir.empty()) return {};
    return resolve_nemotron_directml_model_bin(model_dir);
}

struct NemotronApi {
    using model_create_from_file_fn = decltype(&gptoss_model_create_from_file);
    using model_get_tokenizer_fn = decltype(&gptoss_model_get_tokenizer);
    using model_get_max_context_length_fn = decltype(&gptoss_model_get_max_context_length);
    using model_release_fn = decltype(&gptoss_model_release);
    using tokenizer_get_num_text_tokens_fn = decltype(&gptoss_tokenizer_get_num_text_tokens);
    using tokenizer_get_special_token_id_fn = decltype(&gptoss_tokenizer_get_special_token_id);
    using tokenizer_release_fn = decltype(&gptoss_tokenizer_release);
    using tokenizer_decode_fn = decltype(&gptoss_tokenizer_decode);
    using context_create_fn = decltype(&gptoss_context_create);
    using context_get_num_tokens_fn = decltype(&gptoss_context_get_num_tokens);
    using context_append_tokens_fn = decltype(&gptoss_context_append_tokens);
    using context_append_chars_fn = decltype(&gptoss_context_append_chars);
    using context_sample_fn = decltype(&gptoss_context_sample);
    using context_release_fn = decltype(&gptoss_context_release);

#ifdef _WIN32
    HMODULE handle{nullptr};
#endif

    model_create_from_file_fn model_create_from_file{nullptr};
    model_get_tokenizer_fn model_get_tokenizer{nullptr};
    model_get_max_context_length_fn model_get_max_context_length{nullptr};
    model_release_fn model_release{nullptr};
    tokenizer_get_num_text_tokens_fn tokenizer_get_num_text_tokens{nullptr};
    tokenizer_get_special_token_id_fn tokenizer_get_special_token_id{nullptr};
    tokenizer_release_fn tokenizer_release{nullptr};
    tokenizer_decode_fn tokenizer_decode{nullptr};
    context_create_fn context_create{nullptr};
    context_get_num_tokens_fn context_get_num_tokens{nullptr};
    context_append_tokens_fn context_append_tokens{nullptr};
    context_append_chars_fn context_append_chars{nullptr};
    context_sample_fn context_sample{nullptr};
    context_release_fn context_release{nullptr};

    ~NemotronApi() {
#ifdef _WIN32
        if (handle) {
            FreeLibrary(handle);
            handle = nullptr;
        }
#endif
    }
};

#ifdef _WIN32
template <typename Fn>
bool load_nemotron_symbol(HMODULE handle, const char* name, Fn& out, std::string& error) {
    auto proc = reinterpret_cast<Fn>(GetProcAddress(handle, name));
    if (!proc) {
        error = std::string("missing symbol: ") + name;
        return false;
    }
    out = proc;
    return true;
}

std::shared_ptr<NemotronApi> load_nemotron_api_from_library(const fs::path& path, std::string& error) {
    HMODULE handle = LoadLibraryA(path.string().c_str());
    if (!handle) {
        error = "nemotron DirectML runtime library load failed: " + path.string();
        return nullptr;
    }

    auto api = std::make_shared<NemotronApi>();
    api->handle = handle;

    if (!load_nemotron_symbol(handle, "gptoss_model_create_from_file", api->model_create_from_file, error) ||
        !load_nemotron_symbol(handle, "gptoss_model_get_tokenizer", api->model_get_tokenizer, error) ||
        !load_nemotron_symbol(handle, "gptoss_model_get_max_context_length", api->model_get_max_context_length, error) ||
        !load_nemotron_symbol(handle, "gptoss_model_release", api->model_release, error) ||
        !load_nemotron_symbol(handle, "gptoss_tokenizer_get_num_text_tokens", api->tokenizer_get_num_text_tokens, error) ||
        !load_nemotron_symbol(handle, "gptoss_tokenizer_get_special_token_id", api->tokenizer_get_special_token_id, error) ||
        !load_nemotron_symbol(handle, "gptoss_tokenizer_release", api->tokenizer_release, error) ||
        !load_nemotron_symbol(handle, "gptoss_tokenizer_decode", api->tokenizer_decode, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_create", api->context_create, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_get_num_tokens", api->context_get_num_tokens, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_append_tokens", api->context_append_tokens, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_append_chars", api->context_append_chars, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_sample", api->context_sample, error) ||
        !load_nemotron_symbol(handle, "gptoss_context_release", api->context_release, error)) {
        FreeLibrary(handle);
        api->handle = nullptr;
        return nullptr;
    }

    return api;
}
#endif  // _WIN32

std::shared_ptr<NemotronApi> resolve_nemotron_api(const fs::path& model_dir, std::string& error) {
    static std::mutex mutex;
    static std::weak_ptr<NemotronApi> cached;

    std::lock_guard<std::mutex> lock(mutex);
    if (auto api = cached.lock()) {
        return api;
    }

#ifdef _WIN32
    const char* override_path = std::getenv("LLM_NODE_NEMOTRON_DML_LIB");
    if (override_path && *override_path) {
        fs::path path(override_path);
        if (!fs::exists(path)) {
            error = "nemotron DirectML runtime library not found: " + path.string();
            return nullptr;
        }
        auto api = load_nemotron_api_from_library(path, error);
        if (api) {
            cached = api;
        }
        return api;
    }

    const fs::path model_lib = model_dir.empty() ? fs::path() : (model_dir / "nemotron_directml.dll");
    if (!model_lib.empty() && fs::exists(model_lib)) {
        auto api = load_nemotron_api_from_library(model_lib, error);
        if (api) {
            cached = api;
        }
        return api;
    }

    auto api = load_nemotron_api_from_library("nemotron_directml.dll", error);
    if (!api) {
        error = "nemotron DirectML runtime library not found (set LLM_NODE_NEMOTRON_DML_LIB)";
        return nullptr;
    }
    cached = api;
    return api;
#else
    error = "nemotron DirectML runtime is only supported on Windows";
    return nullptr;
#endif
}
#endif  // USE_GPTOSS

std::optional<fs::path> resolve_model_dir(const ModelDescriptor& descriptor) {
    if (!descriptor.model_dir.empty()) return fs::path(descriptor.model_dir);
    if (!descriptor.primary_path.empty()) {
        return fs::path(descriptor.primary_path).parent_path();
    }
    return std::nullopt;
}

std::optional<std::string> validate_required_metadata(const fs::path& model_dir) {
    const fs::path config_path = model_dir / "config.json";
    if (!is_regular_nonempty_file(config_path)) {
        return std::string("Missing required config.json: ") + config_path.string();
    }
    const fs::path tokenizer_path = model_dir / "tokenizer.json";
    if (!is_regular_nonempty_file(tokenizer_path)) {
        return std::string("Missing required tokenizer.json: ") + tokenizer_path.string();
    }
    return std::nullopt;
}

bool is_index_file(const fs::path& path) {
    const std::string filename = path.filename().string();
    return filename.find(".safetensors.index.json") != std::string::npos;
}

std::optional<nlohmann::json> load_json(const fs::path& path, std::string& err) {
    std::ifstream ifs(path);
    if (!ifs) {
        err = "Failed to open index file: " + path.string();
        return std::nullopt;
    }
    try {
        nlohmann::json j;
        ifs >> j;
        return j;
    } catch (const std::exception& e) {
        err = std::string("Failed to parse JSON: ") + e.what();
        return std::nullopt;
    }
}

std::optional<std::string> find_shard_for_tensor(const nlohmann::json& index,
                                                 const std::string& tensor_name,
                                                 std::string& err) {
    if (!index.is_object()) {
        err = "Index JSON is not an object";
        return std::nullopt;
    }
    if (!index.contains("weight_map") || !index["weight_map"].is_object()) {
        err = "Index JSON missing weight_map";
        return std::nullopt;
    }
    const auto& weight_map = index["weight_map"];
    if (!weight_map.contains(tensor_name) || !weight_map[tensor_name].is_string()) {
        err = "Tensor not found in weight_map: " + tensor_name;
        return std::nullopt;
    }
    return weight_map[tensor_name].get<std::string>();
}

std::optional<std::vector<fs::path>> collect_shards(const nlohmann::json& index,
                                                    const fs::path& model_dir,
                                                    std::string& err) {
    if (!index.is_object()) {
        err = "Index JSON is not an object";
        return std::nullopt;
    }
    if (!index.contains("weight_map") || !index["weight_map"].is_object()) {
        err = "Index JSON missing weight_map";
        return std::nullopt;
    }
    std::vector<fs::path> shards;
    for (const auto& item : index["weight_map"].items()) {
        if (!item.value().is_string()) {
            err = "Index JSON has non-string shard entry";
            return std::nullopt;
        }
        fs::path shard_path(item.value().get<std::string>());
        if (!shard_path.is_absolute()) {
            shard_path = model_dir / shard_path;
        }
        shards.push_back(shard_path);
    }
    if (shards.empty()) {
        err = "Index JSON contains no shard entries";
        return std::nullopt;
    }
    std::sort(shards.begin(), shards.end());
    shards.erase(std::unique(shards.begin(), shards.end()), shards.end());
    return shards;
}

std::optional<size_t> load_max_position_embeddings(const fs::path& model_dir) {
    const auto cfg_path = model_dir / "config.json";
    if (!fs::exists(cfg_path)) return std::nullopt;
    try {
        std::ifstream ifs(cfg_path);
        nlohmann::json j;
        ifs >> j;
        if (j.contains("max_position_embeddings") && j["max_position_embeddings"].is_number_integer()) {
            const auto value = j["max_position_embeddings"].get<long long>();
            if (value > 0) return static_cast<size_t>(value);
        }
    } catch (...) {
        return std::nullopt;
    }
    return std::nullopt;
}

std::string trim_copy(std::string s) {
    auto l = s.find_first_not_of(" \t\n\r");
    if (l == std::string::npos) return "";
    auto r = s.find_last_not_of(" \t\n\r");
    return s.substr(l, r - l + 1);
}

std::string build_nemotron_chat_prompt(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            oss << "System: " << msg.content << "\n\n";
        } else if (msg.role == "user") {
            oss << "User: " << msg.content << "\n\n";
        } else if (msg.role == "assistant") {
            oss << "Assistant: " << msg.content << "\n\n";
        }
    }
    oss << "Assistant: ";
    return oss.str();
}

ModelLoadResult validate_safetensors_file(const fs::path& path, const std::string& expected_tensor) {
    ModelLoadResult result;
    if (!fs::exists(path)) {
        result.error_message = "Safetensors file not found: " + path.string();
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    safetensors::safetensors_t st;
    std::string warn;
    std::string err;
    if (!safetensors::mmap_from_file(path.string(), &st, &warn, &err)) {
        result.error_message = err.empty() ? "Failed to mmap safetensors file" : err;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return result;
    }

    if (!warn.empty()) {
        spdlog::warn("NemotronEngine: safetensors warning: {}", warn);
    }

    std::string validate_err;
    if (!safetensors::validate_data_offsets(st, validate_err)) {
        result.error_message = validate_err.empty() ? "Invalid data_offsets in safetensors" : validate_err;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return result;
    }

    if (!expected_tensor.empty() && !st.tensors.count(expected_tensor)) {
        result.error_message = "Expected tensor not found: " + expected_tensor;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return result;
    }

    result.success = true;
    result.error_code = EngineErrorCode::kOk;
    return result;
}
}  // namespace

struct NemotronEngine::LoadedModel {
    std::string model_path;
#ifdef USE_GPTOSS
    std::shared_ptr<NemotronApi> api;
    gptoss_model_t model{nullptr};
    gptoss_tokenizer_t tokenizer{nullptr};
    size_t max_context{0};
    uint32_t num_text_tokens{0};
    uint32_t end_token_id{0};
    uint32_t return_token_id{0};
    bool has_end_token{false};

    ~LoadedModel() {
        if (api && tokenizer) {
            api->tokenizer_release(tokenizer);
            tokenizer = nullptr;
        }
        if (api && model) {
            api->model_release(model);
            model = nullptr;
        }
    }
#endif
};

std::shared_ptr<NemotronEngine::LoadedModel> NemotronEngine::ensureLoaded(
    const ModelDescriptor& descriptor,
    ModelLoadResult& result) const {
    const std::string key = !descriptor.model_dir.empty() ? descriptor.model_dir : descriptor.primary_path;
    if (key.empty()) {
        result.success = false;
        result.error_message = "Nemotron model directory is empty";
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = loaded_models_.find(key);
        if (it != loaded_models_.end()) {
            result.success = true;
            result.error_code = EngineErrorCode::kOk;
            return it->second;
        }
    }

#if !defined(_WIN32) || !defined(USE_GPTOSS)
    result.success = false;
    result.error_message = "Nemotron DirectML engine requires Windows build with USE_GPTOSS";
    result.error_code = EngineErrorCode::kUnsupported;
    return nullptr;
#else
    const auto model_dir = resolve_model_dir(descriptor);
    if (!model_dir) {
        result.success = false;
        result.error_message = "Nemotron model_dir is empty";
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }
    if (auto missing = validate_required_metadata(*model_dir)) {
        result.success = false;
        result.error_message = *missing;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return nullptr;
    }

    fs::path primary(descriptor.primary_path);
    if (!primary.empty() && !fs::exists(primary)) {
        result.success = false;
        result.error_message = "Primary path not found: " + primary.string();
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    if (!primary.empty() && is_index_file(primary)) {
        std::string err;
        auto index = load_json(primary, err);
        if (!index) {
            result.success = false;
            result.error_message = err;
            result.error_code = EngineErrorCode::kModelCorrupt;
            return nullptr;
        }
        auto shards = collect_shards(*index, *model_dir, err);
        if (!shards) {
            result.success = false;
            result.error_message = err;
            result.error_code = EngineErrorCode::kModelCorrupt;
            return nullptr;
        }
        for (const auto& shard : *shards) {
            if (!is_regular_nonempty_file(shard)) {
                result.success = false;
                result.error_message = "Shard file missing or empty: " + shard.string();
                result.error_code = EngineErrorCode::kModelCorrupt;
                return nullptr;
            }
        }
    } else if (!primary.empty()) {
        if (!is_regular_nonempty_file(primary)) {
            result.success = false;
            result.error_message = "Safetensors file missing or empty: " + primary.string();
            result.error_code = EngineErrorCode::kModelCorrupt;
            return nullptr;
        }
    }

    const fs::path model_file = resolve_nemotron_directml_model_file(descriptor);
    if (model_file.empty()) {
        result.success = false;
        result.error_message =
            "nemotron DirectML model artifact not found (expected model.directml.bin or model.dml.bin)";
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    auto api = resolve_nemotron_api(*model_dir, result.error_message);
    if (!api) {
        result.success = false;
        if (result.error_message.empty()) {
            result.error_message = "nemotron DirectML runtime library not available";
        }
        result.error_code = EngineErrorCode::kUnsupported;
        return nullptr;
    }

    gptoss_model_t model = nullptr;
    enum gptoss_status status = api->model_create_from_file(model_file.string().c_str(), &model);
    if (status != gptoss_status_success || model == nullptr) {
        result.success = false;
        result.error_message = "nemotron model_create_from_file failed: status=" + std::to_string(status);
        if (status == gptoss_status_unsupported_argument) {
            result.error_message += " (unsupported DirectML artifact/layout)";
            result.error_code = EngineErrorCode::kUnsupported;
        } else {
            result.error_code = EngineErrorCode::kLoadFailed;
        }
        return nullptr;
    }

    gptoss_tokenizer_t tokenizer = nullptr;
    status = api->model_get_tokenizer(model, &tokenizer);
    if (status != gptoss_status_success || tokenizer == nullptr) {
        api->model_release(model);
        result.success = false;
        result.error_message = "nemotron model_get_tokenizer failed: status=" + std::to_string(status);
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    size_t max_ctx = 0;
    status = api->model_get_max_context_length(model, &max_ctx);
    if (status != gptoss_status_success) {
        spdlog::warn("NemotronEngine: model_get_max_context_length failed: status={}",
                     static_cast<int>(status));
        max_ctx = 0;
    }

    auto lm = std::make_shared<LoadedModel>();
    lm->model_path = model_file.string();
    lm->api = api;
    lm->model = model;
    lm->tokenizer = tokenizer;
    lm->max_context = max_ctx;

    uint32_t num_text = 0;
    status = api->tokenizer_get_num_text_tokens(tokenizer, &num_text);
    if (status == gptoss_status_success) {
        lm->num_text_tokens = num_text;
    }

    uint32_t end_id = 0;
    status = api->tokenizer_get_special_token_id(tokenizer, gptoss_special_token_end, &end_id);
    if (status == gptoss_status_success) {
        lm->end_token_id = end_id;
        lm->has_end_token = true;
    }
    uint32_t return_id = 0;
    status = api->tokenizer_get_special_token_id(tokenizer, gptoss_special_token_return, &return_id);
    if (status == gptoss_status_success) {
        lm->return_token_id = return_id;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        loaded_models_[key] = lm;
    }

    result.success = true;
    result.error_code = EngineErrorCode::kOk;
    return lm;
#endif
}

NemotronEngine::~NemotronEngine() {
#ifdef USE_CUDA
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& entry : cuda_buffers_) {
        if (entry.second.device_ptr) {
            cudaFree(entry.second.device_ptr);
            entry.second.device_ptr = nullptr;
        }
    }
    cuda_buffers_.clear();
#endif
}

ModelLoadResult NemotronEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;
#if defined(_WIN32) && defined(USE_GPTOSS)
    (void)ensureLoaded(descriptor, result);
    return result;
#else
    if (!descriptor.format.empty() && descriptor.format != "safetensors") {
        result.error_message = "Nemotron engine supports safetensors only";
        result.error_code = EngineErrorCode::kUnsupported;
        return result;
    }
    if (descriptor.primary_path.empty()) {
        result.error_message = "Nemotron primary path is empty";
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    const auto model_dir = resolve_model_dir(descriptor);
    if (!model_dir) {
        result.error_message = "Nemotron model_dir is empty";
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }
    if (auto missing = validate_required_metadata(*model_dir)) {
        result.error_message = *missing;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return result;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (loaded_.count(descriptor.primary_path) != 0) {
            result.success = true;
            result.error_code = EngineErrorCode::kOk;
            return result;
        }
    }

    fs::path primary(descriptor.primary_path);
    if (!fs::exists(primary)) {
        result.error_message = "Primary path not found: " + primary.string();
        result.error_code = EngineErrorCode::kLoadFailed;
        return result;
    }

    if (is_index_file(primary)) {
        std::string err;
        auto index = load_json(primary, err);
        if (!index) {
            result.error_message = err;
            result.error_code = EngineErrorCode::kModelCorrupt;
            return result;
        }
        auto shards = collect_shards(*index, *model_dir, err);
        if (!shards) {
            result.error_message = err;
            result.error_code = EngineErrorCode::kModelCorrupt;
            return result;
        }
        for (const auto& shard : *shards) {
            if (!is_regular_nonempty_file(shard)) {
                result.error_message = "Shard file missing or empty: " + shard.string();
                result.error_code = EngineErrorCode::kModelCorrupt;
                return result;
            }
        }
        auto shard = find_shard_for_tensor(*index, kKnownTensorName, err);
        if (!shard) {
            result.error_message = err;
            result.error_code = EngineErrorCode::kModelCorrupt;
            return result;
        }
        fs::path shard_path(*shard);
        if (!shard_path.is_absolute()) {
            shard_path = primary.parent_path() / shard_path;
        }
        result = validate_safetensors_file(shard_path, kKnownTensorName);
#ifdef USE_CUDA
        if (result.success) {
            const bool upload_enabled = std::getenv("LLM_NODE_NEMOTRON_UPLOAD") != nullptr;
            if (upload_enabled) {
                const size_t max_bytes = parse_env_size_bytes(std::getenv("LLM_NODE_NEMOTRON_UPLOAD_MAX_BYTES"))
                                             .value_or(kDefaultUploadMaxBytes);
                std::string upload_err;
                auto uploaded = upload_tensor_to_gpu(shard_path, kKnownTensorName, max_bytes, upload_err);
                if (!uploaded) {
                    result.success = false;
                    result.error_message = upload_err;
                    result.error_code = EngineErrorCode::kInternal;
                    return result;
                }
                std::lock_guard<std::mutex> lock(mutex_);
                cuda_buffers_[descriptor.primary_path] = {uploaded->device_ptr, uploaded->bytes};
            }
        }
#endif
    } else {
        result = validate_safetensors_file(primary, kKnownTensorName);
#ifdef USE_CUDA
        if (result.success) {
            const bool upload_enabled = std::getenv("LLM_NODE_NEMOTRON_UPLOAD") != nullptr;
            if (upload_enabled) {
                const size_t max_bytes = parse_env_size_bytes(std::getenv("LLM_NODE_NEMOTRON_UPLOAD_MAX_BYTES"))
                                             .value_or(kDefaultUploadMaxBytes);
                std::string upload_err;
                auto uploaded = upload_tensor_to_gpu(primary, kKnownTensorName, max_bytes, upload_err);
                if (!uploaded) {
                    result.success = false;
                    result.error_message = upload_err;
                    result.error_code = EngineErrorCode::kInternal;
                    return result;
                }
                std::lock_guard<std::mutex> lock(mutex_);
                cuda_buffers_[descriptor.primary_path] = {uploaded->device_ptr, uploaded->bytes};
            }
        }
#endif
    }

    if (result.success) {
        std::lock_guard<std::mutex> lock(mutex_);
        loaded_.insert(descriptor.primary_path);
    }

    return result;
#endif
}

std::string NemotronEngine::generateCompletionInternal(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::vector<ChatMessage>* chat_messages,
    const std::function<void(const std::string&)>& on_token) const {
#if !defined(_WIN32) || !defined(USE_GPTOSS)
    (void)prompt;
    (void)descriptor;
    (void)params;
    (void)chat_messages;
    (void)on_token;
    throw std::runtime_error("Nemotron engine does not support text generation on this platform");
#else
    ModelLoadResult load_result;
    auto lm = ensureLoaded(descriptor, load_result);
    if (!load_result.success || !lm) {
        throw std::runtime_error(load_result.error_message.empty()
                                     ? "Failed to load nemotron model"
                                     : load_result.error_message);
    }

    auto api = lm->api;
    if (!api) {
        throw std::runtime_error("Nemotron runtime library not available");
    }

    gptoss_context_t ctx = nullptr;
    enum gptoss_status status = api->context_create(
        lm->model,
        /*context_length=*/0,
        /*max_batch_tokens=*/0,
        &ctx);
    if (status != gptoss_status_success || ctx == nullptr) {
        throw std::runtime_error("nemotron context_create failed: status=" + std::to_string(status));
    }

    struct ContextGuard {
        std::shared_ptr<NemotronApi> api;
        gptoss_context_t ctx{nullptr};
        ~ContextGuard() {
            if (api && ctx) api->context_release(ctx);
        }
    } guard{api, ctx};

    std::string combined_prompt;
    if (chat_messages) {
        combined_prompt = build_nemotron_chat_prompt(*chat_messages);
    } else {
        combined_prompt = prompt;
    }

    if (!combined_prompt.empty()) {
        status = api->context_append_chars(ctx, combined_prompt.c_str(), combined_prompt.size(), nullptr);
        if (status != gptoss_status_success) {
            throw std::runtime_error("nemotron context_append_chars failed: status=" + std::to_string(status));
        }
    }

    size_t prompt_tokens = 0;
    if (api->context_get_num_tokens) {
        size_t num_tokens = 0;
        status = api->context_get_num_tokens(ctx, &num_tokens);
        if (status == gptoss_status_success) {
            prompt_tokens = num_tokens;
        } else {
            spdlog::warn("NemotronEngine: context_get_num_tokens failed: status={}", static_cast<int>(status));
        }
    }

    fs::path model_dir = descriptor.model_dir.empty()
                             ? fs::path(descriptor.primary_path).parent_path()
                             : fs::path(descriptor.model_dir);
    size_t max_context = lm->max_context;
    if (auto cfg_max = load_max_position_embeddings(model_dir)) {
        max_context = *cfg_max;
    }

    size_t effective_max_tokens = resolve_effective_max_tokens(params.max_tokens, prompt_tokens, max_context);
    if (effective_max_tokens == 0) {
        throw std::runtime_error("prompt exceeds model max context");
    }

    const uint64_t seed = resolve_seed(params.seed);
    const float user_temperature = std::clamp(params.temperature, 0.0f, 2.0f);
    const float temperature = user_temperature == 0.0f ? 0.0f : std::clamp(1.0f / user_temperature, 0.0f, 8.0f);

    NemotronTokenEmitState stream_state;
    auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, params.stop_sequences);
    if (!stop_sequences.empty()) {
        stream_state.stop_stream.emplace(std::move(stop_sequences));
    }
    stream_state.output.reserve(effective_max_tokens * 4);

    auto decode_token = [&](uint32_t token) {
        const void* ptr = nullptr;
        size_t sz = 0;
        const auto decode_status = api->tokenizer_decode(lm->tokenizer, token, &ptr, &sz);
        if (decode_status != gptoss_status_success || ptr == nullptr || sz == 0) {
            return std::string();
        }
        return std::string(reinterpret_cast<const char*>(ptr), sz);
    };

    for (size_t i = 0; i < effective_max_tokens && !stream_state.stopped; ++i) {
        uint32_t tok = 0;
        size_t out_len = 0;
        status = api->context_sample(
            ctx,
            temperature,
            seed,
            /*max_tokens=*/1,
            &tok,
            &out_len);
        if (status != gptoss_status_success) {
            throw std::runtime_error("nemotron context_sample failed: status=" + std::to_string(status));
        }
        if (out_len == 0) break;
        if (lm->has_end_token && tok == lm->end_token_id) break;
        if (lm->return_token_id != 0 && tok == lm->return_token_id) break;

        emit_token_metrics(params, tok);
        emit_text_token(tok, lm->num_text_tokens, decode_token, stream_state, nullptr, on_token);
        if (stream_state.stopped) {
            break;
        }
    }

    if (stream_state.stop_stream) {
        auto emit_chunk = [&](const std::string& chunk) {
            if (chunk.empty()) return;
            stream_state.output.append(chunk);
            if (on_token) {
                on_token(chunk);
            }
        };
        stream_state.stop_stream->flush(emit_chunk);
    }

    return trim_copy(std::move(stream_state.output));
#endif
}

std::string NemotronEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
#if defined(_WIN32) && defined(USE_GPTOSS)
    return generateCompletionInternal("", descriptor, params, &messages, {});
#else
    (void)messages;
    (void)descriptor;
    (void)params;
    throw std::runtime_error("Nemotron engine does not support text generation yet");
#endif
}

std::string NemotronEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
#if defined(_WIN32) && defined(USE_GPTOSS)
    return generateCompletionInternal(prompt, descriptor, params, nullptr, {});
#else
    (void)prompt;
    (void)descriptor;
    (void)params;
    throw std::runtime_error("Nemotron engine does not support text generation yet");
#endif
}

std::vector<std::string> NemotronEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
#if defined(_WIN32) && defined(USE_GPTOSS)
    std::vector<std::string> tokens;
    auto token_cb = [&](const std::string& token) {
        tokens.push_back(token);
        if (on_token) {
            on_token(token);
        }
    };
    (void)generateCompletionInternal("", descriptor, params, &messages, token_cb);
    return tokens;
#else
    (void)messages;
    (void)descriptor;
    (void)params;
    (void)on_token;
    throw std::runtime_error("Nemotron engine does not support text generation yet");
#endif
}

std::vector<std::vector<float>> NemotronEngine::generateEmbeddings(
    const std::vector<std::string>&,
    const ModelDescriptor&) const {
    throw std::runtime_error("Nemotron engine does not support embeddings yet");
}

size_t NemotronEngine::getModelMaxContext(const ModelDescriptor& descriptor) const {
    ModelLoadResult load_result;
    auto lm = ensureLoaded(descriptor, load_result);
    if (!load_result.success || !lm) return 0;
#ifdef USE_GPTOSS
    return lm->max_context;
#else
    return 0;
#endif
}

uint64_t NemotronEngine::getModelVramBytes(const ModelDescriptor&) const {
#if !defined(_WIN32) || !defined(USE_GPTOSS)
    (void)descriptor;
    return 0;
#else
    if (descriptor.model_dir.empty()) {
        return 0;
    }
    const fs::path model_dir(descriptor.model_dir);
    const fs::path model_file = resolve_nemotron_directml_model_file(descriptor);
    if (model_file.empty()) {
        return 0;
    }
    std::error_code ec;
    auto size = fs::file_size(model_file, ec);
    if (ec) {
        return 0;
    }
    return static_cast<uint64_t>(size);
#endif
}

}  // namespace llm_node
