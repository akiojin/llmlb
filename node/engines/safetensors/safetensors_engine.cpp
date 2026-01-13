/**
 * @file safetensors_engine.cpp
 * @brief SafetensorsEngine implementation
 *
 * SPEC-69549000: safetensors.cpp Node integration
 */

#include "safetensors_engine.h"

#include <algorithm>
#include <cstring>
#include <filesystem>
#include <sstream>

#include <nlohmann/json.hpp>
#include <safetensors.h>

namespace llm_node {

namespace {

// Buffer size for generated text
constexpr size_t kMaxOutputLength = 32768;
constexpr size_t kMaxPromptLength = 65536;

// Error callback context
struct ErrorContext {
    stcpp_error last_error{STCPP_OK};
    std::string last_message;
};

void errorCallback(stcpp_error error, const char* message, void* user_data) {
    auto* ctx = static_cast<ErrorContext*>(user_data);
    if (ctx) {
        ctx->last_error = error;
        ctx->last_message = message ? message : "";
    }
}

// Stream callback context
struct StreamContext {
    std::function<void(const std::string&)> callback;
    std::vector<std::string> tokens;
    const InferenceParams* params{nullptr};
    bool cancelled{false};
};

bool streamCallback(const char* token_text, int32_t /*token_id*/, void* user_data) {
    auto* ctx = static_cast<StreamContext*>(user_data);
    if (!ctx || ctx->cancelled) {
        return false;  // Stop generation
    }

    if (token_text && ctx->callback) {
        std::string token(token_text);
        ctx->tokens.push_back(token);
        ctx->callback(token);
    }

    // Check abort callback
    if (ctx->params && ctx->params->abort_callback) {
        if (ctx->params->abort_callback(ctx->params->abort_callback_ctx)) {
            ctx->cancelled = true;
            return false;
        }
    }

    return true;  // Continue generation
}

// Detect GPU backend from system
stcpp_backend_type detectBackend() {
#if defined(STCPP_USE_METAL)
    return STCPP_BACKEND_METAL;
#elif defined(STCPP_USE_CUDA)
    return STCPP_BACKEND_CUDA;
#elif defined(STCPP_USE_ROCM)
    return STCPP_BACKEND_ROCM;
#elif defined(STCPP_USE_VULKAN)
    return STCPP_BACKEND_VULKAN;
#else
    return STCPP_BACKEND_CPU;
#endif
}

}  // namespace

SafetensorsEngine::SafetensorsEngine(const std::string& models_dir)
    : models_dir_(models_dir) {
    stcpp_init();
}

SafetensorsEngine::~SafetensorsEngine() {
    std::lock_guard<std::mutex> lock(models_mutex_);
    for (auto& [name, loaded] : loaded_models_) {
        if (loaded) {
            if (loaded->ctx) {
                stcpp_context_free(loaded->ctx);
            }
            if (loaded->model) {
                stcpp_model_free(loaded->model);
            }
        }
    }
    loaded_models_.clear();
    stcpp_free();
}

std::string SafetensorsEngine::runtime() const {
    return "safetensors_cpp";
}

bool SafetensorsEngine::supportsTextGeneration() const {
    return true;
}

bool SafetensorsEngine::supportsEmbeddings() const {
    return true;
}

ModelLoadResult SafetensorsEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;

    std::lock_guard<std::mutex> lock(models_mutex_);

    // Check if already loaded
    auto it = loaded_models_.find(descriptor.name);
    if (it != loaded_models_.end() && it->second) {
        result.success = true;
        result.error_code = EngineErrorCode::kOk;
        return result;
    }

    // Determine model directory (stcpp_model_load expects model directory, not file path)
    std::string model_dir = descriptor.model_dir;
    if (model_dir.empty()) {
        // Fall back to extracting directory from primary_path
        if (!descriptor.primary_path.empty()) {
            model_dir = std::filesystem::path(descriptor.primary_path).parent_path().string();
        } else {
            model_dir = models_dir_ + "/" + descriptor.name;
        }
    }

    // Verify config.json exists in model directory
    const auto config_path = std::filesystem::path(model_dir) / "config.json";
    if (!std::filesystem::exists(config_path)) {
        result.success = false;
        result.error_code = EngineErrorCode::kLoadFailed;
        result.error_message = "config.json not found in: " + model_dir;
        return result;
    }

    // Load model (pass model directory, not file path)
    fprintf(stderr, "[DEBUG] SafetensorsEngine::loadModel: loading model from %s\n", model_dir.c_str());
    fflush(stderr);

    ErrorContext error_ctx;
    stcpp_model* model = stcpp_model_load(model_dir.c_str(), errorCallback, &error_ctx);
    if (!model) {
        result.success = false;
        result.error_code = EngineErrorCode::kLoadFailed;
        result.error_message = "Failed to load model: " + error_ctx.last_message;
        return result;
    }

    fprintf(stderr, "[DEBUG] SafetensorsEngine::loadModel: model loaded, creating context\n");
    fflush(stderr);

    // Create context
    stcpp_context_params ctx_params = stcpp_context_default_params();
    ctx_params.backend = detectBackend();
    if (ctx_params.backend == STCPP_BACKEND_CPU) {
        result.success = false;
        result.error_code = EngineErrorCode::kUnsupported;
        result.error_message = "GPU backend not available (build without Metal/CUDA/ROCm/Vulkan)";
        return result;
    }
    ctx_params.n_gpu_layers = -1;  // All layers on GPU

    fprintf(stderr, "[DEBUG] SafetensorsEngine::loadModel: calling stcpp_context_new\n");
    fflush(stderr);

    stcpp_context* ctx = stcpp_context_new(model, ctx_params);

    fprintf(stderr, "[DEBUG] SafetensorsEngine::loadModel: stcpp_context_new returned ctx=%p\n", (void*)ctx);
    fflush(stderr);

    if (!ctx) {
        stcpp_model_free(model);
        result.success = false;
        result.error_code = EngineErrorCode::kOomVram;
        result.error_message = "Failed to create context (likely VRAM insufficient)";
        return result;
    }

    // Get tokenizer
    stcpp_tokenizer* tokenizer = stcpp_model_get_tokenizer(model);

    // Store loaded model
    auto loaded = std::make_unique<LoadedModel>();
    loaded->model = model;
    loaded->ctx = ctx;
    loaded->tokenizer = tokenizer;
    loaded->max_context = static_cast<size_t>(stcpp_model_max_context(model));

    // Get VRAM usage
    stcpp_vram_usage vram = stcpp_context_vram_usage(ctx);
    loaded->vram_bytes = vram.total_bytes;

    loaded_models_[descriptor.name] = std::move(loaded);

    fprintf(stderr, "[DEBUG] SafetensorsEngine::loadModel: model stored, returning success\n");
    fflush(stderr);

    result.success = true;
    result.error_code = EngineErrorCode::kOk;
    return result;
}

SafetensorsEngine::LoadedModel* SafetensorsEngine::getOrLoadModel(
    const ModelDescriptor& descriptor) const {
    std::lock_guard<std::mutex> lock(models_mutex_);

    auto it = loaded_models_.find(descriptor.name);
    if (it != loaded_models_.end() && it->second) {
        return it->second.get();
    }

    // Model not loaded - should have been loaded via loadModel()
    return nullptr;
}

std::string SafetensorsEngine::buildChatPrompt(
    const std::vector<ChatMessage>& messages,
    stcpp_tokenizer* tokenizer) const {
    // Build JSON array of messages
    nlohmann::json messages_json = nlohmann::json::array();
    for (const auto& msg : messages) {
        messages_json.push_back({{"role", msg.role}, {"content", msg.content}});
    }

    std::string json_str = messages_json.dump();

    // Apply chat template
    std::vector<char> output(kMaxPromptLength);
    int32_t len = stcpp_apply_chat_template(
        tokenizer, json_str.c_str(), output.data(),
        static_cast<int32_t>(output.size()), true);

    if (len > 0) {
        return std::string(output.data(), static_cast<size_t>(len));
    }

    // Fallback: simple concatenation
    std::ostringstream oss;
    for (const auto& msg : messages) {
        oss << msg.role << ": " << msg.content << "\n";
    }
    return oss.str();
}

void SafetensorsEngine::convertSamplingParams(const InferenceParams& params,
                                              void* out_params) {
    auto* sp = static_cast<stcpp_sampling_params*>(out_params);
    *sp = stcpp_sampling_default_params();
    sp->temperature = params.temperature;
    sp->top_p = params.top_p;
    sp->top_k = params.top_k;
    sp->repeat_penalty = params.repeat_penalty;
    sp->presence_penalty = params.presence_penalty;
    sp->frequency_penalty = params.frequency_penalty;
    sp->seed = static_cast<int32_t>(params.seed);
}

std::string SafetensorsEngine::generateChat(const std::vector<ChatMessage>& messages,
                                            const ModelDescriptor& descriptor,
                                            const InferenceParams& params) const {
    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateChat: entered for model %s\n", descriptor.name.c_str());
    fflush(stderr);

    auto* loaded = getOrLoadModel(descriptor);
    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateChat: getOrLoadModel returned %p\n", (void*)loaded);
    fflush(stderr);
    if (!loaded) {
        return "";
    }

    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateChat: calling buildChatPrompt\n");
    fflush(stderr);

    std::string prompt = buildChatPrompt(messages, loaded->tokenizer);

    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateChat: prompt built (len=%zu), calling generateCompletion\n", prompt.length());
    fflush(stderr);

    return generateCompletion(prompt, descriptor, params);
}

std::string SafetensorsEngine::generateCompletion(const std::string& prompt,
                                                  const ModelDescriptor& descriptor,
                                                  const InferenceParams& params) const {
    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateCompletion: entered\n");
    fflush(stderr);

    auto* loaded = getOrLoadModel(descriptor);
    if (!loaded) {
        fprintf(stderr, "[DEBUG] SafetensorsEngine::generateCompletion: model not loaded\n");
        fflush(stderr);
        return "";
    }

    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateCompletion: model loaded, ctx=%p\n", (void*)loaded->ctx);
    fflush(stderr);

    stcpp_sampling_params sp;
    convertSamplingParams(params, &sp);

    size_t max_tokens = params.max_tokens > 0 ? params.max_tokens : kDefaultMaxTokens;

    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateCompletion: calling stcpp_generate with max_tokens=%zu\n", max_tokens);
    fflush(stderr);

    std::vector<char> output(kMaxOutputLength);
    stcpp_error err = stcpp_generate(
        loaded->ctx, prompt.c_str(), sp,
        static_cast<int32_t>(max_tokens),
        output.data(), static_cast<int32_t>(output.size()));

    fprintf(stderr, "[DEBUG] SafetensorsEngine::generateCompletion: stcpp_generate returned %d\n", err);
    fflush(stderr);

    if (err != STCPP_OK) {
        return "";
    }

    return std::string(output.data());
}

std::vector<std::string> SafetensorsEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const ModelDescriptor& descriptor,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
    auto* loaded = getOrLoadModel(descriptor);
    if (!loaded) {
        return {};
    }

    std::string prompt = buildChatPrompt(messages, loaded->tokenizer);

    stcpp_sampling_params sp;
    convertSamplingParams(params, &sp);

    size_t max_tokens = params.max_tokens > 0 ? params.max_tokens : kDefaultMaxTokens;

    StreamContext stream_ctx;
    stream_ctx.callback = on_token;
    stream_ctx.params = &params;

    stcpp_error err = stcpp_generate_stream(
        loaded->ctx, prompt.c_str(), sp,
        static_cast<int32_t>(max_tokens),
        streamCallback, &stream_ctx);

    if (err != STCPP_OK && err != STCPP_ERROR_CANCELLED) {
        return {};
    }

    return stream_ctx.tokens;
}

std::vector<std::vector<float>> SafetensorsEngine::generateEmbeddings(
    const std::vector<std::string>& inputs,
    const ModelDescriptor& descriptor) const {
    auto* loaded = getOrLoadModel(descriptor);
    if (!loaded) {
        return {};
    }

    int32_t dims = stcpp_embeddings_dims(loaded->model);
    if (dims <= 0) {
        return {};
    }

    std::vector<std::vector<float>> results;
    results.reserve(inputs.size());

    for (const auto& input : inputs) {
        std::vector<float> embedding(static_cast<size_t>(dims));
        stcpp_error err = stcpp_embeddings(loaded->ctx, input.c_str(),
                                           embedding.data(), dims);
        if (err == STCPP_OK) {
            results.push_back(std::move(embedding));
        } else {
            results.emplace_back();  // Empty vector for failed embedding
        }
    }

    return results;
}

size_t SafetensorsEngine::getModelMaxContext(const ModelDescriptor& descriptor) const {
    auto* loaded = getOrLoadModel(descriptor);
    if (!loaded) {
        return 0;
    }
    return loaded->max_context;
}

uint64_t SafetensorsEngine::getModelVramBytes(const ModelDescriptor& descriptor) const {
    auto* loaded = getOrLoadModel(descriptor);
    if (!loaded) {
        return 0;
    }
    return loaded->vram_bytes;
}

}  // namespace llm_node
