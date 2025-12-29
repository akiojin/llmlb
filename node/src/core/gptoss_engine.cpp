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

#ifdef USE_GPTOSS
extern "C" {
#include <gpt-oss/functions.h>
#include <gpt-oss/types.h>
}
#endif

namespace fs = std::filesystem;

namespace llm_node {

namespace {
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
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                if (tokens.size() >= max_tokens) break;
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty() && tokens.size() < max_tokens) {
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

struct GptOssEngine::LoadedModel {
    std::string model_path;
#ifdef USE_GPTOSS
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
        if (tokenizer) {
            gptoss_tokenizer_release(tokenizer);
            tokenizer = nullptr;
        }
        if (model) {
            gptoss_model_release(model);
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
        return nullptr;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (auto it = loaded_.find(key); it != loaded_.end()) {
            result.success = true;
            return it->second;
        }
    }

#ifndef USE_GPTOSS
    result.success = false;
    result.error_message = "gpt-oss engine requires Metal build (USE_GPTOSS)";
    return nullptr;
#else
    std::string validation_error;
    if (!validate_safetensors_files(descriptor, validation_error)) {
        result.success = false;
        result.error_message = validation_error;
        return nullptr;
    }
    const fs::path model_dir(descriptor.model_dir);
    const fs::path model_bin = resolve_gptoss_metal_model_bin(model_dir);
    if (model_bin.empty()) {
        result.success = false;
        result.error_message =
            "gpt-oss Metal model artifact not found (expected model.metal.bin or metal/model.bin)";
        return nullptr;
    }

    gptoss_model_t model = nullptr;
    enum gptoss_status status = gptoss_model_create_from_file(model_bin.string().c_str(), &model);
    if (status != gptoss_status_success || model == nullptr) {
        result.success = false;
        result.error_message = "gptoss_model_create_from_file failed: status=" + std::to_string(status);
        return nullptr;
    }

    gptoss_tokenizer_t tokenizer = nullptr;
    status = gptoss_model_get_tokenizer(model, &tokenizer);
    if (status != gptoss_status_success || tokenizer == nullptr) {
        gptoss_model_release(model);
        result.success = false;
        result.error_message = "gptoss_model_get_tokenizer failed: status=" + std::to_string(status);
        return nullptr;
    }

    size_t max_ctx = 0;
    status = gptoss_model_get_max_context_length(model, &max_ctx);
    if (status != gptoss_status_success) {
        spdlog::warn("GptOssEngine: gptoss_model_get_max_context_length failed: status={}", static_cast<int>(status));
        max_ctx = 0;
    }

    auto lm = std::make_shared<LoadedModel>();
    lm->model_path = model_bin.string();
    lm->model = model;
    lm->tokenizer = tokenizer;
    lm->max_context = max_ctx;

    uint32_t num_text = 0;
    status = gptoss_tokenizer_get_num_text_tokens(tokenizer, &num_text);
    if (status == gptoss_status_success) {
        lm->num_text_tokens = num_text;
    }

    auto load_special = [&](enum gptoss_special_token ty, uint32_t& out) {
        uint32_t id = 0;
        enum gptoss_status st = gptoss_tokenizer_get_special_token_id(tokenizer, ty, &id);
        if (st == gptoss_status_success) out = id;
    };

    load_special(gptoss_special_token_start, lm->start_token_id);
    load_special(gptoss_special_token_message, lm->message_token_id);
    load_special(gptoss_special_token_channel, lm->channel_token_id);
    load_special(gptoss_special_token_return, lm->return_token_id);

    uint32_t end_id = 0;
    status = gptoss_tokenizer_get_special_token_id(tokenizer, gptoss_special_token_end, &end_id);
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
    return lm;
#endif
}

ModelLoadResult GptOssEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;
    (void)ensureLoaded(descriptor, result);
    return result;
}

std::string GptOssEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
    return generateCompletion("", descriptor, params, &messages);
}

std::string GptOssEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {
    return generateCompletion(prompt, descriptor, params, nullptr);
}

std::string GptOssEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::vector<ChatMessage>* chat_messages) const {

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
    gptoss_context_t ctx = nullptr;
    enum gptoss_status status = gptoss_context_create(
        lm->model,
        /*context_length=*/0,
        /*max_batch_tokens=*/0,
        &ctx);
    if (status != gptoss_status_success || ctx == nullptr) {
        throw std::runtime_error("gptoss_context_create failed: status=" + std::to_string(status));
    }

    // Ensure release even when exceptions happen.
    struct ContextGuard {
        gptoss_context_t ctx{nullptr};
        ~ContextGuard() {
            if (ctx) gptoss_context_release(ctx);
        }
    } guard{ctx};

    // Build Harmony prompt by appending special tokens + strings (matches upstream Metal example).
    const std::string system_prompt = build_gptoss_system_prompt_text(chat_messages ? *chat_messages : std::vector<ChatMessage>{});

    auto append_token = [&](uint32_t tok) {
        status = gptoss_context_append_tokens(ctx, 1, &tok);
        if (status != gptoss_status_success) {
            throw std::runtime_error("gptoss_context_append_tokens failed: status=" + std::to_string(status));
        }
    };

    auto append_text = [&](const std::string& s) {
        status = gptoss_context_append_chars(ctx, s.c_str(), s.size(), nullptr);
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

    const size_t max_tokens = params.max_tokens == 0 ? 1 : params.max_tokens;
    const uint64_t seed = resolve_seed(params.seed);
    // gpt-oss Metal reference uses `exp((logit - max) * temperature)` i.e. this parameter is 1/T.
    // Convert OpenAI-style temperature (T) into inverse temperature for the kernel.
    const float user_temperature = std::clamp(params.temperature, 0.0f, 2.0f);
    const float temperature = user_temperature == 0.0f ? 0.0f : std::clamp(1.0f / user_temperature, 0.0f, 8.0f);

    std::string final_bytes;
    final_bytes.reserve(max_tokens * 4);

    const bool trace = std::getenv("LLM_NODE_GPTOSS_TRACE") != nullptr;
    size_t trace_tokens_logged = 0;
    bool saw_return_token = false;

    for (size_t i = 0; i < max_tokens; ++i) {
        uint32_t tok = 0;
        size_t out_len = 0;
        status = gptoss_context_sample(
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

        // text tokens
        if (lm->num_text_tokens > 0 && tok < lm->num_text_tokens) {
            const void* ptr = nullptr;
            size_t sz = 0;
            status = gptoss_tokenizer_decode(lm->tokenizer, tok, &ptr, &sz);
            if (status != gptoss_status_success || ptr == nullptr || sz == 0) {
                continue;
            }
            final_bytes.append(reinterpret_cast<const char*>(ptr), sz);
        }

        if (trace && trace_tokens_logged < 64) {
            spdlog::info("GptOssEngine trace: tok={}", tok);
            trace_tokens_logged++;
        }
    }

    if (trace) {
        spdlog::info(
            "GptOssEngine trace summary: max_tokens={} saw_return_token={} final_bytes={}",
            max_tokens,
            saw_return_token,
            final_bytes.size());
    }

    return trim_copy(std::move(final_bytes));
#endif
}

std::vector<std::string> GptOssEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
    const std::string text = generateChat(messages, descriptor, params);
    auto tokens = split_whitespace_tokens(text, params.max_tokens);
    for (const auto& t : tokens) {
        if (on_token) on_token(t);
    }
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

}  // namespace llm_node
