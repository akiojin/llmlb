#include "core/gptoss_engine.h"

#include <algorithm>
#include <chrono>
#include <cctype>
#include <filesystem>
#include <random>
#include <sstream>
#include <stdexcept>

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
#ifdef USE_GPTOSS
std::string trim_copy(std::string s) {
    auto l = s.find_first_not_of(" \t\n\r");
    if (l == std::string::npos) return "";
    auto r = s.find_last_not_of(" \t\n\r");
    return s.substr(l, r - l + 1);
}
#endif  // USE_GPTOSS

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

std::string strip_control_tokens(std::string text) {
    const std::vector<std::string> tokens = {
        "<|start|>", "<|end|>", "<|message|>", "<|channel|>",
        "<|startoftext|>", "<|endoftext|>", "<|return|>", "<|call|>",
        "<|constrain|>", "<|endofprompt|>",
        "<|im_start|>", "<|im_end|>", "<s>", "</s>", "<|eot_id|>"
    };
    for (const auto& t : tokens) {
        size_t pos = 0;
        while ((pos = text.find(t, pos)) != std::string::npos) {
            text.erase(pos, t.size());
        }
    }
    return trim_copy(std::move(text));
}
#endif  // USE_GPTOSS

// gpt-oss chat template (minimal, keeps user content unchanged)
std::string build_gptoss_chat_prompt(const std::vector<ChatMessage>& messages) {
    std::ostringstream oss;

    bool has_system = false;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            has_system = true;
            break;
        }
    }

    if (!has_system) {
        oss << "<|start|>system<|message|>You are a helpful assistant.\n\nReasoning: none<|end|>";
    }

    for (const auto& msg : messages) {
        if (msg.role == "system") {
            oss << "<|start|>system<|message|>" << msg.content << "\n\nReasoning: none<|end|>";
        } else if (msg.role == "user" || msg.role == "assistant") {
            oss << "<|start|>" << msg.role << "<|message|>" << msg.content << "<|end|>";
        } else {
            // Unknown role → treat as user
            oss << "<|start|>user<|message|>" << msg.content << "<|end|>";
        }
    }

    oss << "<|start|>assistant<|channel|>final<|message|>";
    return oss.str();
}

}  // namespace

struct GptOssEngine::LoadedModel {
    std::string model_path;
#ifdef USE_GPTOSS
    gptoss_model_t model{nullptr};
    gptoss_tokenizer_t tokenizer{nullptr};
    size_t max_context{0};
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

    uint32_t end_id = 0;
    status = gptoss_tokenizer_get_special_token_id(tokenizer, gptoss_special_token_end, &end_id);
    if (status == gptoss_status_success) {
        lm->end_token_id = end_id;
        lm->has_end_token = true;
    }

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

    ModelLoadResult load_result;
    auto model = ensureLoaded(descriptor, load_result);
    if (!load_result.success || !model) {
        throw std::runtime_error(load_result.error_message.empty()
                                     ? "Failed to load gpt-oss model"
                                     : load_result.error_message);
    }

    const std::string prompt = build_gptoss_chat_prompt(messages);
    return generateCompletion(prompt, descriptor, params);
}

std::string GptOssEngine::generateCompletion(
    const std::string& prompt,
    const ModelDescriptor& descriptor,
    const InferenceParams& params) const {

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

    size_t appended = 0;
    status = gptoss_context_append_chars(ctx, prompt.c_str(), prompt.size(), &appended);
    if (status != gptoss_status_success) {
        throw std::runtime_error("gptoss_context_append_chars failed: status=" + std::to_string(status));
    }

    const uint64_t seed = resolve_seed(params.seed);
    const size_t max_tokens = params.max_tokens == 0 ? 1 : params.max_tokens;
    std::vector<uint32_t> out_tokens(max_tokens);
    size_t out_len = 0;
    status = gptoss_context_sample(
        ctx,
        params.temperature,
        seed,
        max_tokens,
        out_tokens.data(),
        &out_len);
    if (status != gptoss_status_success) {
        throw std::runtime_error("gptoss_context_sample failed: status=" + std::to_string(status));
    }
    out_tokens.resize(out_len);

    std::string text;
    text.reserve(out_tokens.size() * 4);
    for (uint32_t tok : out_tokens) {
        if (lm->has_end_token && tok == lm->end_token_id) {
            break;
        }
        const void* ptr = nullptr;
        size_t sz = 0;
        status = gptoss_tokenizer_decode(lm->tokenizer, tok, &ptr, &sz);
        if (status != gptoss_status_success || ptr == nullptr || sz == 0) {
            continue;
        }
        text.append(reinterpret_cast<const char*>(ptr), sz);
    }

    return strip_control_tokens(std::move(text));
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
