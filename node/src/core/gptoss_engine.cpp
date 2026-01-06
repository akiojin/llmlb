#include "core/gptoss_engine.h"

#include <algorithm>
#include <chrono>
#include <cctype>
#include <cstdlib>
#include <ctime>
#include <filesystem>
#include <fstream>
#include <optional>
#include <random>
#include <sstream>
#include <stdexcept>
#include <unordered_set>

#include <spdlog/spdlog.h>
#include "utils/stop_sequences.h"

#ifdef _WIN32
#include <windows.h>
#endif

#ifdef USE_GPTOSS
extern "C" {
#include <gpt-oss/functions.h>
#include <gpt-oss/types.h>
}
#endif

namespace fs = std::filesystem;

namespace llm_node {

namespace {
static const std::vector<std::string> kDefaultStopSequences = {
    "<|im_end|>",
    "<|end|>",
    "<|start|>",
    "<|eot_id|>",
    "</s>",
    "<|endoftext|>",
};

struct GptOssTokenEmitState {
    std::string output;
    std::optional<StopSequenceStream> stop_stream;
    bool stopped{false};
};

void emit_text_token(uint32_t token,
                     uint32_t num_text_tokens,
                     const std::function<std::string(uint32_t)>& decode,
                     GptOssTokenEmitState& state,
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

uint64_t steady_now_ns() {
    return static_cast<uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(
            std::chrono::steady_clock::now().time_since_epoch()).count());
}

void emit_token_metrics(const InferenceParams& params, uint32_t token_id) {
    if (!params.on_token_callback) return;
    params.on_token_callback(params.on_token_callback_ctx, token_id, steady_now_ns());
}

bool is_safetensors_index_file(const fs::path& path) {
    const std::string filename = path.filename().string();
    const std::string suffix = ".safetensors.index.json";
    if (filename.size() < suffix.size()) return false;
    std::string lower = filename;
    std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return lower.rfind(suffix) == lower.size() - suffix.size();
}

std::optional<std::vector<std::string>> load_safetensors_index_shards(const fs::path& index_path) {
    if (!fs::exists(index_path)) return std::nullopt;
    try {
        std::ifstream ifs(index_path);
        nlohmann::json j;
        ifs >> j;

        if (!j.contains("weight_map") || !j["weight_map"].is_object()) {
            return std::nullopt;
        }

        const auto& weight_map = j["weight_map"];
        std::unordered_set<std::string> shard_set;
        for (auto it = weight_map.begin(); it != weight_map.end(); ++it) {
            if (!it.value().is_string()) continue;
            shard_set.insert(it.value().get<std::string>());
        }
        std::vector<std::string> shards(shard_set.begin(), shard_set.end());
        std::sort(shards.begin(), shards.end());
        return shards;
    } catch (...) {
        return std::nullopt;
    }
}

bool validate_safetensors_files(const ModelDescriptor& descriptor, std::string& error) {
    if (descriptor.format != "safetensors") return true;

    fs::path model_dir = descriptor.model_dir.empty()
                             ? fs::path(descriptor.primary_path).parent_path()
                             : fs::path(descriptor.model_dir);
    if (model_dir.empty()) {
        error = "model_dir is required for safetensors models";
        return false;
    }

    if (!fs::exists(model_dir / "config.json")) {
        error = "config.json is required for safetensors models";
        return false;
    }
    if (!fs::exists(model_dir / "tokenizer.json")) {
        error = "tokenizer.json is required for safetensors models";
        return false;
    }

    std::vector<std::string> shards;
    std::optional<std::string> index_name;

    if (descriptor.metadata && descriptor.metadata->contains("safetensors")) {
        const auto& meta = (*descriptor.metadata)["safetensors"];
        if (meta.contains("index") && meta["index"].is_string()) {
            index_name = meta["index"].get<std::string>();
        }
        if (meta.contains("shards") && meta["shards"].is_array()) {
            for (const auto& shard : meta["shards"]) {
                if (shard.is_string()) {
                    shards.push_back(shard.get<std::string>());
                }
            }
        }
    }

    fs::path primary = descriptor.primary_path.empty()
                           ? fs::path()
                           : fs::path(descriptor.primary_path);

    if (shards.empty()) {
        if (!primary.empty() && is_safetensors_index_file(primary)) {
            auto parsed = load_safetensors_index_shards(primary);
            if (!parsed) {
                error = "invalid safetensors index (missing weight_map)";
                return false;
            }
            shards = *parsed;
            if (!primary.filename().empty()) {
                index_name = primary.filename().string();
            }
        } else if (!primary.empty()) {
            shards.push_back(primary.filename().string());
        }
    }

    if (index_name) {
        const auto index_path = model_dir / *index_name;
        if (!fs::exists(index_path)) {
            error = "missing safetensors index: " + *index_name;
            return false;
        }
    }

    for (const auto& shard : shards) {
        if (shard.empty()) continue;
        const auto shard_path = model_dir / shard;
        if (!fs::exists(shard_path)) {
            error = "missing safetensors shard: " + shard;
            return false;
        }
    }

    return true;
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

std::string current_utc_date_yyyy_mm_dd() {
    std::time_t now = std::time(nullptr);
    std::tm tm{};
#if defined(_WIN32)
    gmtime_s(&tm, &now);
#else
    tm = *std::gmtime(&now);
#endif
    char buf[16];
    if (std::strftime(buf, sizeof(buf), "%Y-%m-%d", &tm) == 0) {
        return "1970-01-01";
    }
    return std::string(buf);
}

std::vector<std::string> split_whitespace_tokens(const std::string& text, size_t max_tokens) {
    std::vector<std::string> tokens;
    std::string current;
    const size_t effective_max_tokens = max_tokens == 0 ? kDefaultMaxTokens : max_tokens;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                if (tokens.size() >= effective_max_tokens) break;
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty() && tokens.size() < effective_max_tokens) {
        tokens.push_back(current);
    }
    return tokens;
}

#ifdef USE_GPTOSS
uint64_t resolve_seed(uint32_t seed) {
    if (seed != 0) return seed;
    const uint64_t t = static_cast<uint64_t>(
        std::chrono::steady_clock::now().time_since_epoch().count());
    // SplitMix64-ish scrambler
    uint64_t x = t + UINT64_C(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)) * UINT64_C(0x94d049bb133111eb);
    return x ^ (x >> 31);
}

fs::path resolve_gptoss_metal_model_bin(const fs::path& model_dir) {
    // Router caches the artifact as a flat file to work with the registry file API.
    const fs::path p1 = model_dir / "model.metal.bin";
    if (fs::exists(p1)) return p1;

    // Allow manual placement following HF repo structure.
    const fs::path p2 = model_dir / "metal" / "model.bin";
    if (fs::exists(p2)) return p2;

    // Last resort: plain model.bin in the root.
    const fs::path p3 = model_dir / "model.bin";
    if (fs::exists(p3)) return p3;

    return {};
}

fs::path resolve_gptoss_directml_model_bin(const fs::path& model_dir) {
    const fs::path p1 = model_dir / "model.directml.bin";
    if (fs::exists(p1)) return p1;
    const fs::path p2 = model_dir / "model.dml.bin";
    if (fs::exists(p2)) return p2;
    return {};
}

fs::path resolve_gptoss_directml_model_file(const ModelDescriptor& descriptor) {
    fs::path model_dir = descriptor.model_dir.empty()
                             ? fs::path(descriptor.primary_path).parent_path()
                             : fs::path(descriptor.model_dir);
    if (model_dir.empty()) return {};
    return resolve_gptoss_directml_model_bin(model_dir);
}

struct GptOssApi {
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

    ~GptOssApi() {
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
bool load_gptoss_symbol(HMODULE handle, const char* name, Fn& out, std::string& error) {
    auto proc = reinterpret_cast<Fn>(GetProcAddress(handle, name));
    if (!proc) {
        error = std::string("missing symbol: ") + name;
        return false;
    }
    out = proc;
    return true;
}

std::shared_ptr<GptOssApi> load_gptoss_api_from_library(const fs::path& path, std::string& error) {
    HMODULE handle = LoadLibraryA(path.string().c_str());
    if (!handle) {
        error = "gpt-oss DirectML runtime library load failed: " + path.string();
        return nullptr;
    }

    auto api = std::make_shared<GptOssApi>();
    api->handle = handle;

    if (!load_gptoss_symbol(handle, "gptoss_model_create_from_file", api->model_create_from_file, error) ||
        !load_gptoss_symbol(handle, "gptoss_model_get_tokenizer", api->model_get_tokenizer, error) ||
        !load_gptoss_symbol(handle, "gptoss_model_get_max_context_length", api->model_get_max_context_length, error) ||
        !load_gptoss_symbol(handle, "gptoss_model_release", api->model_release, error) ||
        !load_gptoss_symbol(handle, "gptoss_tokenizer_get_num_text_tokens", api->tokenizer_get_num_text_tokens, error) ||
        !load_gptoss_symbol(handle, "gptoss_tokenizer_get_special_token_id", api->tokenizer_get_special_token_id, error) ||
        !load_gptoss_symbol(handle, "gptoss_tokenizer_release", api->tokenizer_release, error) ||
        !load_gptoss_symbol(handle, "gptoss_tokenizer_decode", api->tokenizer_decode, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_create", api->context_create, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_get_num_tokens", api->context_get_num_tokens, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_append_tokens", api->context_append_tokens, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_append_chars", api->context_append_chars, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_sample", api->context_sample, error) ||
        !load_gptoss_symbol(handle, "gptoss_context_release", api->context_release, error)) {
        FreeLibrary(handle);
        api->handle = nullptr;
        return nullptr;
    }

    return api;
}
#endif  // _WIN32

std::shared_ptr<GptOssApi> resolve_gptoss_api(const fs::path& model_dir, std::string& error) {
    static std::mutex mutex;
    static std::weak_ptr<GptOssApi> cached;

    std::lock_guard<std::mutex> lock(mutex);
    if (auto api = cached.lock()) {
        return api;
    }

#ifdef _WIN32
    const char* override_path = std::getenv("LLM_NODE_GPTOSS_DML_LIB");
    if (override_path && *override_path) {
        fs::path path(override_path);
        if (!fs::exists(path)) {
            error = "gpt-oss DirectML runtime library not found: " + path.string();
            return nullptr;
        }
        auto api = load_gptoss_api_from_library(path, error);
        if (api) {
            cached = api;
        }
        return api;
    }

    const fs::path model_lib = model_dir.empty() ? fs::path() : (model_dir / "gptoss_directml.dll");
    if (!model_lib.empty() && fs::exists(model_lib)) {
        auto api = load_gptoss_api_from_library(model_lib, error);
        if (api) {
            cached = api;
        }
        return api;
    }

    auto api = load_gptoss_api_from_library("gptoss_directml.dll", error);
    if (!api) {
        error = "gpt-oss DirectML runtime library not found (set LLM_NODE_GPTOSS_DML_LIB)";
        return nullptr;
    }
    cached = api;
    return api;
#else
    auto api = std::make_shared<GptOssApi>();
    api->model_create_from_file = &gptoss_model_create_from_file;
    api->model_get_tokenizer = &gptoss_model_get_tokenizer;
    api->model_get_max_context_length = &gptoss_model_get_max_context_length;
    api->model_release = &gptoss_model_release;
    api->tokenizer_get_num_text_tokens = &gptoss_tokenizer_get_num_text_tokens;
    api->tokenizer_get_special_token_id = &gptoss_tokenizer_get_special_token_id;
    api->tokenizer_release = &gptoss_tokenizer_release;
    api->tokenizer_decode = &gptoss_tokenizer_decode;
    api->context_create = &gptoss_context_create;
    api->context_get_num_tokens = &gptoss_context_get_num_tokens;
    api->context_append_tokens = &gptoss_context_append_tokens;
    api->context_append_chars = &gptoss_context_append_chars;
    api->context_sample = &gptoss_context_sample;
    api->context_release = &gptoss_context_release;
    cached = api;
    return api;
#endif
}

#endif  // USE_GPTOSS

// gpt-oss requires the Harmony response format (https://github.com/openai/harmony).
// The official Metal reference implementation composes the prompt by appending special tokens
// (start/message/end/channel/return) and text segments to the context (not by concatenating a big string).
std::string build_gptoss_system_prompt_text(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;

    oss << "You are ChatGPT, a large language model trained by OpenAI.\n";
    oss << "Knowledge cutoff: 2024-06\n";
    oss << "Current date: " << current_utc_date_yyyy_mm_dd() << "\n\n";
    // Metal reference uses "reasoning effort <level>" (lowercase).
    oss << "reasoning effort high\n\n";
    oss << "# Valid channels: analysis, final. Channel must be included for every message.";

    bool has_system = false;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            has_system = true;
            break;
        }
    }
    if (has_system) {
        oss << "\n\n";
        for (const auto& msg : messages) {
            if (msg.role != "system") continue;
            oss << msg.content << "\n";
        }
    }

    return oss.str();
}

}  // namespace

std::string emitGptOssTextTokensForTest(const std::vector<uint32_t>& tokens,
                                        uint32_t num_text_tokens,
                                        const std::function<std::string(uint32_t)>& decode,
                                        std::vector<std::string>* emitted,
                                        const std::function<void(const std::string&)>& on_token) {
    GptOssTokenEmitState state;
    for (const auto& token : tokens) {
        emit_text_token(token, num_text_tokens, decode, state, emitted, on_token);
    }
    return state.output;
}

struct GptOssEngine::LoadedModel {
    std::string model_path;
#ifdef USE_GPTOSS
    std::shared_ptr<GptOssApi> api;
    gptoss_model_t model{nullptr};
    gptoss_tokenizer_t tokenizer{nullptr};
    size_t max_context{0};
    uint32_t num_text_tokens{0};
    uint32_t start_token_id{0};
    uint32_t message_token_id{0};
    uint32_t channel_token_id{0};
    uint32_t return_token_id{0};
    uint32_t end_token_id{0};
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

std::shared_ptr<GptOssEngine::LoadedModel> GptOssEngine::ensureLoaded(
    const ModelDescriptor& descriptor,
    ModelLoadResult& result) const {

    const std::string key = !descriptor.model_dir.empty() ? descriptor.model_dir : descriptor.primary_path;
    if (key.empty()) {
        result.success = false;
        result.error_message = "Model directory is empty";
        result.error_code = EngineErrorCode::kUnsupported;
        return nullptr;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (auto it = loaded_.find(key); it != loaded_.end()) {
            result.success = true;
            result.error_code = EngineErrorCode::kOk;
            return it->second;
        }
    }

#ifndef USE_GPTOSS
    result.success = false;
    result.error_message = "gpt-oss engine requires Metal build (USE_GPTOSS)";
    result.error_code = EngineErrorCode::kUnsupported;
    return nullptr;
#else
    std::string validation_error;
    if (!validate_safetensors_files(descriptor, validation_error)) {
        result.success = false;
        result.error_message = validation_error;
        result.error_code = EngineErrorCode::kModelCorrupt;
        return nullptr;
    }
    fs::path model_dir = descriptor.model_dir.empty()
                             ? fs::path(descriptor.primary_path).parent_path()
                             : fs::path(descriptor.model_dir);
    const fs::path model_file =
#if defined(_WIN32)
        resolve_gptoss_directml_model_file(descriptor);
#else
        resolve_gptoss_metal_model_bin(model_dir);
#endif
    if (model_file.empty()) {
        result.success = false;
        result.error_message =
#if defined(_WIN32)
            "gpt-oss DirectML model artifact not found (expected model.directml.bin or model.dml.bin)";
#else
            "gpt-oss Metal model artifact not found (expected model.metal.bin or metal/model.bin)";
#endif
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    auto api = resolve_gptoss_api(model_dir, result.error_message);
    if (!api) {
        result.success = false;
        if (result.error_message.empty()) {
            result.error_message = "gpt-oss runtime library not available";
        }
        result.error_code = EngineErrorCode::kUnsupported;
        return nullptr;
    }

    gptoss_model_t model = nullptr;
    enum gptoss_status status = api->model_create_from_file(model_file.string().c_str(), &model);
    if (status != gptoss_status_success || model == nullptr) {
        result.success = false;
        result.error_message = "gptoss_model_create_from_file failed: status=" + std::to_string(status);
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
        result.error_message = "gptoss_model_get_tokenizer failed: status=" + std::to_string(status);
        result.error_code = EngineErrorCode::kLoadFailed;
        return nullptr;
    }

    size_t max_ctx = 0;
    status = api->model_get_max_context_length(model, &max_ctx);
    if (status != gptoss_status_success) {
        spdlog::warn("GptOssEngine: gptoss_model_get_max_context_length failed: status={}", static_cast<int>(status));
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

    auto load_special = [&](enum gptoss_special_token ty, uint32_t& out) {
        uint32_t id = 0;
        enum gptoss_status st = api->tokenizer_get_special_token_id(tokenizer, ty, &id);
        if (st == gptoss_status_success) out = id;
    };

    load_special(gptoss_special_token_start, lm->start_token_id);
    load_special(gptoss_special_token_message, lm->message_token_id);
    load_special(gptoss_special_token_channel, lm->channel_token_id);
    load_special(gptoss_special_token_return, lm->return_token_id);

    uint32_t end_id = 0;
    status = api->tokenizer_get_special_token_id(tokenizer, gptoss_special_token_end, &end_id);
    if (status == gptoss_status_success) {
        lm->end_token_id = end_id;
        lm->has_end_token = true;
    }

    auto validate_special = [&](const char* name, uint32_t id) {
        if (id == 0) {
            throw std::runtime_error(std::string("gpt-oss tokenizer missing special token id: ") + name);
        }
        if (lm->num_text_tokens > 0 && id < lm->num_text_tokens) {
            throw std::runtime_error(std::string("gpt-oss special token id is in text token range: ") + name);
        }
    };

    validate_special("start", lm->start_token_id);
    validate_special("message", lm->message_token_id);
    validate_special("channel", lm->channel_token_id);
    validate_special("return", lm->return_token_id);
    if (lm->has_end_token) {
        validate_special("end", lm->end_token_id);
    } else {
        throw std::runtime_error("gpt-oss tokenizer missing special token: end");
    }

    spdlog::info(
        "GptOssEngine tokenizer loaded: num_text_tokens={} start={} message={} channel={} return={} end={}",
        lm->num_text_tokens,
        lm->start_token_id,
        lm->message_token_id,
        lm->channel_token_id,
        lm->return_token_id,
        lm->end_token_id);

    {
        std::lock_guard<std::mutex> lock(mutex_);
        loaded_[key] = lm;
    }

    result.success = true;
    result.error_code = EngineErrorCode::kOk;
    return lm;
#endif
}

ModelLoadResult GptOssEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;
    (void)ensureLoaded(descriptor, result);
    if (result.success) {
        result.error_code = EngineErrorCode::kOk;
    }
    return result;
}

std::string GptOssEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
    return generateCompletion("", descriptor, params, &messages, {});
}

std::string GptOssEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
    return generateCompletion(prompt, descriptor, params, nullptr, {});
}

std::string GptOssEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::vector<ChatMessage>* chat_messages,
    const std::function<void(const std::string&)>& on_token) const {

    ModelLoadResult load_result;
    auto lm = ensureLoaded(descriptor, load_result);
    if (!load_result.success || !lm) {
        throw std::runtime_error(load_result.error_message.empty()
                                     ? "Failed to load gpt-oss model"
                                     : load_result.error_message);
    }

#ifndef USE_GPTOSS
    (void)prompt;
    (void)params;
    throw std::runtime_error("gpt-oss engine requires Metal build (USE_GPTOSS)");
#else
    auto api = lm->api;
    if (!api) {
        throw std::runtime_error("gpt-oss runtime library not available");
    }

    gptoss_context_t ctx = nullptr;
    enum gptoss_status status = api->context_create(
        lm->model,
        /*context_length=*/0,
        /*max_batch_tokens=*/0,
        &ctx);
    if (status != gptoss_status_success || ctx == nullptr) {
        throw std::runtime_error("gptoss_context_create failed: status=" + std::to_string(status));
    }

    // Ensure release even when exceptions happen.
    struct ContextGuard {
        std::shared_ptr<GptOssApi> api;
        gptoss_context_t ctx{nullptr};
        ~ContextGuard() {
            if (api && ctx) api->context_release(ctx);
        }
    } guard{api, ctx};

    // Build Harmony prompt by appending special tokens + strings (matches upstream Metal example).
    const std::string system_prompt = build_gptoss_system_prompt_text(chat_messages ? *chat_messages : std::vector<ChatMessage>{});

    auto append_token = [&](uint32_t tok) {
        status = api->context_append_tokens(ctx, 1, &tok);
        if (status != gptoss_status_success) {
            throw std::runtime_error("gptoss_context_append_tokens failed: status=" + std::to_string(status));
        }
    };

    auto append_text = [&](const std::string& s) {
        status = api->context_append_chars(ctx, s.c_str(), s.size(), nullptr);
        if (status != gptoss_status_success) {
            throw std::runtime_error("gptoss_context_append_chars failed: status=" + std::to_string(status));
        }
    };

    // Start-of-text token: gpt-oss tokenizer places it at the first special token slot.
    // This matches HF's tokenizer.json for openai/gpt-oss-* (id == num_text_tokens).
    if (lm->num_text_tokens > 0) {
        append_token(lm->num_text_tokens);
    }

    // system
    append_token(lm->start_token_id);
    append_text("system");
    append_token(lm->message_token_id);
    append_text(system_prompt);
    append_token(lm->end_token_id);

    // messages
    std::vector<ChatMessage> fallback_messages;
    if (!chat_messages) {
        fallback_messages.push_back({"user", prompt});
        chat_messages = &fallback_messages;
    }

    for (const auto& msg : *chat_messages) {
        if (msg.role == "system") {
            continue;  // already integrated into system_prompt
        }
        if (msg.role == "assistant") {
            // Assistant history is encoded as a final channel message.
            append_token(lm->start_token_id);
            append_text("assistant");
            append_token(lm->channel_token_id);
            append_text("final");
            append_token(lm->message_token_id);
            append_text(msg.content);
            append_token(lm->end_token_id);
            continue;
        }

        // user (and unknown)
        append_token(lm->start_token_id);
        append_text("user");
        append_token(lm->message_token_id);
        append_text(msg.content);
        append_token(lm->end_token_id);
    }

    // Generation prefix: <|start|>assistant<|channel|>final<|message|>
    append_token(lm->start_token_id);
    append_text("assistant");
    append_token(lm->channel_token_id);
    append_text("final");
    append_token(lm->message_token_id);

    size_t prompt_tokens = 0;
    if (api->context_get_num_tokens) {
        size_t num_tokens = 0;
        status = api->context_get_num_tokens(ctx, &num_tokens);
        if (status == gptoss_status_success) {
            prompt_tokens = num_tokens;
        } else {
            spdlog::warn("GptOssEngine: gptoss_context_get_num_tokens failed: status={}", static_cast<int>(status));
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
    // gpt-oss Metal reference uses `exp((logit - max) * temperature)` i.e. this parameter is 1/T.
    // Convert OpenAI-style temperature (T) into inverse temperature for the kernel.
    const float user_temperature = std::clamp(params.temperature, 0.0f, 2.0f);
    const float temperature = user_temperature == 0.0f ? 0.0f : std::clamp(1.0f / user_temperature, 0.0f, 8.0f);

    GptOssTokenEmitState stream_state;
    auto stop_sequences = merge_stop_sequences(kDefaultStopSequences, params.stop_sequences);
    if (!stop_sequences.empty()) {
        stream_state.stop_stream.emplace(std::move(stop_sequences));
    }
    stream_state.output.reserve(effective_max_tokens * 4);

    const bool trace = std::getenv("LLM_NODE_GPTOSS_TRACE") != nullptr;
    size_t trace_tokens_logged = 0;
    bool saw_return_token = false;

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
            throw std::runtime_error("gptoss_context_sample failed: status=" + std::to_string(status));
        }
        if (out_len == 0) break;

        if (tok == lm->return_token_id) {
            saw_return_token = true;
            break;
        }
        if (tok == lm->end_token_id) break;
        if (tok == lm->start_token_id) break;

        emit_token_metrics(params, tok);

        emit_text_token(tok, lm->num_text_tokens, decode_token, stream_state, nullptr, on_token);
        if (stream_state.stopped) {
            break;
        }

        if (trace && trace_tokens_logged < 64) {
            spdlog::info("GptOssEngine trace: tok={}", tok);
            trace_tokens_logged++;
        }
    }

    if (trace) {
        spdlog::info(
            "GptOssEngine trace summary: max_tokens={} saw_return_token={} final_bytes={}",
            effective_max_tokens,
            saw_return_token,
            stream_state.output.size());
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

std::vector<std::string> GptOssEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
    std::vector<std::string> tokens;
    auto token_cb = [&](const std::string& token) {
        tokens.push_back(token);
        if (on_token) {
            on_token(token);
        }
    };
    (void)generateCompletion("", descriptor, params, &messages, token_cb);
    return tokens;
}

std::vector<std::vector<float>> GptOssEngine::generateEmbeddings(
    const std::vector<std::string>&,
    const ModelDescriptor&) const {
    throw std::runtime_error("gpt-oss engine does not support embeddings");
}

size_t GptOssEngine::getModelMaxContext(const ModelDescriptor& descriptor) const {
    ModelLoadResult load_result;
    auto lm = ensureLoaded(descriptor, load_result);
    if (!load_result.success || !lm) return 0;
#ifdef USE_GPTOSS
    return lm->max_context;
#else
    return 0;
#endif
}

uint64_t GptOssEngine::getModelVramBytes(const ModelDescriptor& descriptor) const {
#ifndef USE_GPTOSS
    (void)descriptor;
    return 0;
#else
    const fs::path model_dir = descriptor.model_dir.empty()
                                   ? fs::path(descriptor.primary_path).parent_path()
                                   : fs::path(descriptor.model_dir);
    const fs::path model_file =
#if defined(_WIN32)
        resolve_gptoss_directml_model_file(descriptor);
#else
        resolve_gptoss_metal_model_bin(model_dir);
#endif
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
