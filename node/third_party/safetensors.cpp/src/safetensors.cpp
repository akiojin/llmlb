/**
 * @file safetensors.cpp
 * @brief safetensors.cpp core implementation
 */

#include "safetensors.h"
#include "safetensors_internal.h"
#include "ggml_model.h"
#include <cstring>
#include <atomic>
#include <memory>
#include <vector>
#include <cmath>
#include <random>
#include <algorithm>

/* Internal state */
static std::atomic<bool> g_initialized{false};
static stcpp_log_callback g_log_callback = nullptr;
static void* g_log_user_data = nullptr;
static stcpp_log_level g_log_level = STCPP_LOG_INFO;

/* Internal model structure wrapping GgmlModel */
struct stcpp_model_impl {
    std::unique_ptr<stcpp::GgmlModel> ggml_model;
    std::unique_ptr<stcpp::TokenizerImpl> tokenizer;
    std::string name;
};

/* Internal context structure wrapping GgmlContext */
struct stcpp_context_impl {
    std::unique_ptr<stcpp::GgmlContext> ggml_ctx;
    stcpp_model_impl* model;
};

/* Version string */
static const char* VERSION_STRING = "0.1.0";

/* Initialization / Cleanup */

void stcpp_init(void) {
    if (g_initialized.exchange(true)) {
        return;  // Already initialized
    }
    // TODO: Initialize ggml backend
}

void stcpp_free(void) {
    if (!g_initialized.exchange(false)) {
        return;  // Not initialized
    }
    g_log_callback = nullptr;
    g_log_user_data = nullptr;
    // TODO: Cleanup ggml backend
}

void stcpp_free_string(char* str) {
    if (str != nullptr) {
        delete[] str;
    }
}

const char* stcpp_version(void) {
    return VERSION_STRING;
}

int32_t stcpp_abi_version(void) {
    return STCPP_ABI_VERSION;
}

void stcpp_set_log_callback(stcpp_log_callback callback, void* user_data) {
    g_log_callback = callback;
    g_log_user_data = user_data;
}

void stcpp_set_log_level(stcpp_log_level level) {
    g_log_level = level;
}

/* Default parameters */

stcpp_context_params stcpp_context_default_params(void) {
    stcpp_context_params params;
    params.n_ctx = 2048;  // Match contract test expectation
    params.n_batch = 512;
    params.n_threads = 0;  // Auto-detect (0 = auto)
    params.n_gpu_layers = -1;  // All
    params.device_id = 0;
    params.use_mmap = true;  // Match contract test expectation
    params.use_mlock = false;
    params.kv_cache_quant = false;
#if defined(STCPP_USE_METAL)
    // Metal backend currently has kernel compilation issues - use CPU until fixed
    params.backend = STCPP_BACKEND_CPU;
#elif defined(STCPP_USE_CUDA)
    params.backend = STCPP_BACKEND_CUDA;
#elif defined(STCPP_USE_ROCM)
    params.backend = STCPP_BACKEND_ROCM;
#elif defined(STCPP_USE_VULKAN)
    params.backend = STCPP_BACKEND_VULKAN;
#else
    params.backend = STCPP_BACKEND_METAL;  // Default
#endif
    return params;
}

stcpp_sampling_params stcpp_sampling_default_params(void) {
    stcpp_sampling_params params;
    params.temperature = 1.0f;
    params.top_p = 1.0f;
    params.top_k = -1;  // Disabled
    params.min_p = 0.0f;
    params.repeat_penalty = 1.0f;
    params.presence_penalty = 0.0f;
    params.frequency_penalty = 0.0f;
    params.seed = -1;  // Random
    return params;
}

/* Model implementations */

stcpp_model* stcpp_model_load(
    const char* path,
    stcpp_error_callback error_cb,
    void* user_data
) {
    if (!path) {
        if (error_cb) {
            error_cb(STCPP_ERROR_INVALID_MODEL, "Path is null", user_data);
        }
        return nullptr;
    }

    auto model = std::make_unique<stcpp_model_impl>();
    std::string error;

    // Determine backend from default params
    stcpp_context_params default_params = stcpp_context_default_params();

    // Load ggml model
    model->ggml_model.reset(stcpp::load_ggml_model(
        path,
        default_params.backend,
        default_params.device_id,
        error
    ));

    if (!model->ggml_model) {
        if (error_cb) {
            error_cb(STCPP_ERROR_INVALID_MODEL, error.c_str(), user_data);
        }
        return nullptr;
    }

    // Load tokenizer
    model->tokenizer = std::make_unique<stcpp::TokenizerImpl>();
    if (!stcpp::load_tokenizer(path, *model->tokenizer, error)) {
        if (error_cb) {
            error_cb(STCPP_ERROR_INVALID_MODEL, error.c_str(), user_data);
        }
        return nullptr;
    }

    // Extract model name from path
    std::string path_str(path);
    size_t last_slash = path_str.find_last_of("/\\");
    model->name = (last_slash != std::string::npos)
        ? path_str.substr(last_slash + 1)
        : path_str;

    return reinterpret_cast<stcpp_model*>(model.release());
}

void stcpp_model_free(stcpp_model* model) {
    if (model) {
        delete reinterpret_cast<stcpp_model_impl*>(model);
    }
}

const char* stcpp_model_name(const stcpp_model* model) {
    if (!model) return nullptr;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->name.c_str();
}

int32_t stcpp_model_n_layers(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_layer : 0;
}

int32_t stcpp_model_n_heads(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_head : 0;
}

int32_t stcpp_model_hidden_size(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_embd : 0;
}

int32_t stcpp_model_vocab_size(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_vocab : 0;
}

int32_t stcpp_model_max_context(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_ctx_train : 0;
}

stcpp_vram_estimate stcpp_model_estimate_vram(
    const char* path,
    stcpp_backend_type backend,
    int32_t device_id
) {
    stcpp_vram_estimate estimate;
    estimate.vram_required = 0;
    estimate.vram_available = 0;
    estimate.can_load = false;

    if (!path) return estimate;

    // Load hyperparameters to estimate memory
    stcpp::ModelHParams hparams;
    std::string error;
    if (!stcpp::load_hparams(path, hparams, error)) {
        return estimate;
    }

    // Estimate weight memory
    size_t weight_mem = 0;
    weight_mem += (size_t)hparams.n_vocab * hparams.n_embd * 2;  // Embeddings (FP16)

    const int32_t head_dim = hparams.n_embd / hparams.n_head;
    for (int i = 0; i < hparams.n_layer; ++i) {
        // Q, K, V, O projections
        weight_mem += (size_t)hparams.n_embd * hparams.n_head * head_dim * 2 * 4;
        // FFN
        weight_mem += (size_t)hparams.n_embd * hparams.n_ff * 2 * 3;
        // Norms
        weight_mem += hparams.n_embd * 4 * 2;
    }
    // LM head
    weight_mem += (size_t)hparams.n_embd * hparams.n_vocab * 2;

    estimate.vram_required = weight_mem;

    // Query available VRAM
    estimate.vram_available = stcpp_device_vram_free(backend, device_id);
    estimate.can_load = (estimate.vram_available >= estimate.vram_required);

    return estimate;
}

/* Context implementations */

stcpp_context* stcpp_context_new(
    stcpp_model* model,
    stcpp_context_params params
) {
    if (!model) return nullptr;

    auto* model_impl = reinterpret_cast<stcpp_model_impl*>(model);
    if (!model_impl->ggml_model) return nullptr;

    auto ctx = std::make_unique<stcpp_context_impl>();
    ctx->model = model_impl;

    std::string error;
    ctx->ggml_ctx.reset(stcpp::create_ggml_context(
        model_impl->ggml_model.get(),
        params,
        error
    ));

    if (!ctx->ggml_ctx) {
        return nullptr;
    }

    return reinterpret_cast<stcpp_context*>(ctx.release());
}

void stcpp_context_free(stcpp_context* ctx) {
    if (ctx) {
        delete reinterpret_cast<stcpp_context_impl*>(ctx);
    }
}

void stcpp_context_kv_cache_clear(stcpp_context* ctx) {
    if (ctx) {
        auto* impl = reinterpret_cast<stcpp_context_impl*>(ctx);
        if (impl->ggml_ctx) {
            stcpp::clear_kv_cache(impl->ggml_ctx.get());
        }
    }
}

/* VRAM monitoring (Task 59) */

stcpp_vram_usage stcpp_context_vram_usage(const stcpp_context* ctx) {
    stcpp_vram_usage usage = {};

    if (!ctx) return usage;

    auto* impl = reinterpret_cast<const stcpp_context_impl*>(ctx);
    if (!impl->ggml_ctx || !impl->model) return usage;

    auto* ggml_ctx = impl->ggml_ctx.get();
    auto* ggml_model = impl->model->ggml_model.get();

    // Calculate weights memory
    if (ggml_model->buffer) {
        usage.weights_bytes = ggml_backend_buffer_get_size(ggml_model->buffer);
    }

    // Calculate KV cache memory
    // KV cache size = 2 * n_layers * n_ctx * n_embd * dtype_size
    size_t kv_elem_size = 2;  // FP16 default
    if (ggml_ctx->params.kv_cache_quant) {
        kv_elem_size = 1;  // INT8 or FP8
    }
    usage.kv_cache_bytes = 2 * ggml_model->hparams.n_layer *
                           ggml_ctx->kv_size *
                           ggml_model->hparams.n_embd *
                           kv_elem_size;

    // Estimate compute buffer
    usage.compute_bytes = stcpp::estimate_compute_buffer_size(
        ggml_model->hparams,
        ggml_ctx->params.n_ctx,
        ggml_ctx->params.n_batch
    );

    // Total
    usage.total_bytes = usage.weights_bytes + usage.kv_cache_bytes + usage.compute_bytes;
    usage.peak_bytes = usage.total_bytes;  // Track peak in actual usage

    // KV cache ratio
    if (usage.total_bytes > 0) {
        usage.kv_cache_ratio = static_cast<float>(usage.kv_cache_bytes) /
                               static_cast<float>(usage.total_bytes);
    }

    return usage;
}

size_t stcpp_context_kv_cache_size(const stcpp_context* ctx) {
    if (!ctx) return 0;

    auto* impl = reinterpret_cast<const stcpp_context_impl*>(ctx);
    if (!impl->ggml_ctx) return 0;

    return static_cast<size_t>(impl->ggml_ctx->kv_size);
}

float stcpp_context_kv_cache_utilization(const stcpp_context* ctx) {
    if (!ctx) return 0.0f;

    auto* impl = reinterpret_cast<const stcpp_context_impl*>(ctx);
    if (!impl->ggml_ctx || impl->ggml_ctx->kv_size <= 0) return 0.0f;

    return static_cast<float>(impl->ggml_ctx->kv_used) /
           static_cast<float>(impl->ggml_ctx->kv_size);
}

/* Tokenizer implementations */

stcpp_tokenizer* stcpp_model_get_tokenizer(stcpp_model* model) {
    if (!model) return nullptr;
    auto* impl = reinterpret_cast<stcpp_model_impl*>(model);
    return reinterpret_cast<stcpp_tokenizer*>(impl->tokenizer.get());
}

int32_t stcpp_tokenize(
    const stcpp_tokenizer* tokenizer,
    const char* text,
    int32_t* tokens,
    int32_t max_tokens,
    bool add_special
) {
    if (!tokenizer || !text || !tokens || max_tokens <= 0) return 0;

    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);
    std::vector<int32_t> result;
    std::string error;

    if (!stcpp::tokenize(*tok, text, result, add_special, error)) {
        return -1;
    }

    int32_t count = std::min(static_cast<int32_t>(result.size()), max_tokens);
    if (tokens) {
        std::memcpy(tokens, result.data(), count * sizeof(int32_t));
    }
    return static_cast<int32_t>(result.size());
}

int32_t stcpp_detokenize(
    const stcpp_tokenizer* tokenizer,
    const int32_t* tokens,
    int32_t n_tokens,
    char* text,
    int32_t max_length
) {
    if (!tokenizer || !tokens || !text || n_tokens <= 0 || max_length <= 0) return 0;

    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);
    std::vector<int32_t> token_vec(tokens, tokens + n_tokens);
    std::string result;
    std::string error;

    if (!stcpp::detokenize(*tok, token_vec, result, error)) {
        return -1;
    }

    int32_t len = std::min(static_cast<int32_t>(result.size()), max_length - 1);
    if (text) {
        std::memcpy(text, result.data(), len);
        text[len] = '\0';
    }
    return static_cast<int32_t>(result.size());
}

int32_t stcpp_apply_chat_template(
    const stcpp_tokenizer* tokenizer,
    const char* messages_json,
    char* output,
    int32_t max_length,
    bool add_generation_prompt
) {
    if (!tokenizer || !messages_json || !output || max_length <= 0) return 0;

    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);

    // Parse chat template and apply
    stcpp::ChatTemplate tmpl;
    std::string error;

    if (!stcpp::parse_chat_template(tok->chat_template, tmpl, error)) {
        return -1;
    }

    // Parse messages JSON (simplified)
    std::vector<stcpp::ChatMessage> messages;
    // TODO: Parse messages_json into messages vector

    std::string result;
    if (!stcpp::apply_chat_template(tmpl, messages, result, error, add_generation_prompt)) {
        return -1;
    }

    int32_t len = std::min(static_cast<int32_t>(result.size()), max_length - 1);
    std::memcpy(output, result.data(), len);
    output[len] = '\0';
    return static_cast<int32_t>(result.size());
}

int32_t stcpp_token_bos(const stcpp_tokenizer* tokenizer) {
    if (!tokenizer) return -1;
    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);
    return tok->bos_token_id;
}

int32_t stcpp_token_eos(const stcpp_tokenizer* tokenizer) {
    if (!tokenizer) return -1;
    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);
    return tok->eos_token_id;
}

int32_t stcpp_token_pad(const stcpp_tokenizer* tokenizer) {
    if (!tokenizer) return -1;
    auto* tok = reinterpret_cast<const stcpp::TokenizerImpl*>(tokenizer);
    return tok->pad_token_id;
}

/* Sampling helper functions */

static int32_t sample_token(
    const float* logits,
    int32_t n_vocab,
    stcpp_sampling_params params
) {
    // Debug: Check logits state
    float max_logit = *std::max_element(logits, logits + n_vocab);
    int32_t max_idx = static_cast<int32_t>(std::max_element(logits, logits + n_vocab) - logits);
    float min_logit = *std::min_element(logits, logits + n_vocab);

    // Check for NaN/Inf
    int nan_count = 0, inf_count = 0;
    for (int32_t i = 0; i < std::min(n_vocab, 1000); ++i) {
        if (std::isnan(logits[i])) nan_count++;
        if (std::isinf(logits[i])) inf_count++;
    }

    fprintf(stderr, "[DEBUG] sample_token: n_vocab=%d, max_logit=%.6f@%d, min_logit=%.6f, nan=%d, inf=%d\n",
            n_vocab, max_logit, max_idx, min_logit, nan_count, inf_count);

    // Print top 5 logits
    std::vector<std::pair<float, int32_t>> top_logits(n_vocab);
    for (int32_t i = 0; i < n_vocab; ++i) {
        top_logits[i] = {logits[i], i};
    }
    std::partial_sort(top_logits.begin(), top_logits.begin() + 5, top_logits.end(),
                     std::greater<std::pair<float, int32_t>>());
    fprintf(stderr, "[DEBUG] sample_token: top5 logits: ");
    for (int i = 0; i < 5; ++i) {
        fprintf(stderr, "[%d]=%.4f ", top_logits[i].second, top_logits[i].first);
    }
    fprintf(stderr, "\n");
    fflush(stderr);

    // Apply temperature
    std::vector<float> probs(n_vocab);

    float sum = 0.0f;
    for (int32_t i = 0; i < n_vocab; ++i) {
        float p = std::exp((logits[i] - max_logit) / std::max(params.temperature, 0.01f));
        probs[i] = p;
        sum += p;
    }

    // Normalize
    for (int32_t i = 0; i < n_vocab; ++i) {
        probs[i] /= sum;
    }

    // Top-k filtering
    if (params.top_k > 0 && params.top_k < n_vocab) {
        std::vector<std::pair<float, int32_t>> sorted_probs(n_vocab);
        for (int32_t i = 0; i < n_vocab; ++i) {
            sorted_probs[i] = {probs[i], i};
        }
        std::partial_sort(sorted_probs.begin(), sorted_probs.begin() + params.top_k,
                         sorted_probs.end(), std::greater<std::pair<float, int32_t>>());

        std::fill(probs.begin(), probs.end(), 0.0f);
        for (int32_t i = 0; i < params.top_k; ++i) {
            probs[sorted_probs[i].second] = sorted_probs[i].first;
        }
    }

    // Top-p (nucleus) filtering
    if (params.top_p < 1.0f && params.top_p > 0.0f) {
        std::vector<std::pair<float, int32_t>> sorted_probs(n_vocab);
        for (int32_t i = 0; i < n_vocab; ++i) {
            sorted_probs[i] = {probs[i], i};
        }
        std::sort(sorted_probs.begin(), sorted_probs.end(),
                 std::greater<std::pair<float, int32_t>>());

        float cumsum = 0.0f;
        for (int32_t i = 0; i < n_vocab; ++i) {
            cumsum += sorted_probs[i].first;
            if (cumsum > params.top_p) {
                for (int32_t j = i + 1; j < n_vocab; ++j) {
                    probs[sorted_probs[j].second] = 0.0f;
                }
                break;
            }
        }
    }

    // Renormalize
    sum = 0.0f;
    for (int32_t i = 0; i < n_vocab; ++i) {
        sum += probs[i];
    }
    for (int32_t i = 0; i < n_vocab; ++i) {
        probs[i] /= sum;
    }

    // Sample from distribution
    std::random_device rd;
    std::mt19937 gen(params.seed >= 0 ? static_cast<unsigned int>(params.seed) : rd());
    std::discrete_distribution<int32_t> dist(probs.begin(), probs.end());

    return dist(gen);
}

/* Inference implementations */

stcpp_error stcpp_generate(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    char* output,
    int32_t max_output_length
) {
    fprintf(stderr, "[DEBUG] stcpp_generate: entered, prompt='%.50s...', max_tokens=%d\n",
            prompt ? prompt : "NULL", max_tokens);
    fflush(stderr);

    // Validate input
    if (ctx == nullptr) {
        return STCPP_ERROR_INVALID_MODEL;
    }
    if (prompt == nullptr || output == nullptr || max_output_length <= 0) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    auto* ctx_impl = reinterpret_cast<stcpp_context_impl*>(ctx);
    if (!ctx_impl->ggml_ctx || !ctx_impl->model) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    stcpp::GgmlContext* ggml_ctx = ctx_impl->ggml_ctx.get();
    stcpp::GgmlModel* model = ctx_impl->model->ggml_model.get();
    stcpp::TokenizerImpl* tokenizer = ctx_impl->model->tokenizer.get();

    fprintf(stderr, "[DEBUG] stcpp_generate: contexts retrieved, model=%p, tokenizer=%p\n",
            (void*)model, (void*)tokenizer);
    fflush(stderr);

    if (!model || !tokenizer) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    // Tokenize prompt
    // Note: add_bos=false because chat template already starts with <|im_start|>
    fprintf(stderr, "[DEBUG] stcpp_generate: tokenizing prompt...\n");
    fflush(stderr);
    std::vector<int32_t> tokens;
    std::string error;
    if (!stcpp::tokenize(*tokenizer, prompt, tokens, false, error)) {
        fprintf(stderr, "[DEBUG] stcpp_generate: tokenization failed: %s\n", error.c_str());
        fflush(stderr);
        return STCPP_ERROR_INVALID_MODEL;
    }
    fprintf(stderr, "[DEBUG] stcpp_generate: tokenized, n_tokens=%zu\n", tokens.size());
    fflush(stderr);

    // Check context size
    if (static_cast<int32_t>(tokens.size()) + max_tokens > ggml_ctx->kv_size) {
        fprintf(stderr, "[DEBUG] stcpp_generate: context overflow, tokens=%zu + max=%d > kv_size=%d\n",
                tokens.size(), max_tokens, ggml_ctx->kv_size);
        fflush(stderr);
        return STCPP_ERROR_OUT_OF_MEMORY;
    }

    // Reset cancel flag
    ggml_ctx->cancel_flag.store(false, std::memory_order_release);

    // Generation loop
    std::vector<int32_t> generated_tokens;
    std::vector<float> logits(model->hparams.n_vocab);
    int32_t n_past = 0;

    // Process prompt
    fprintf(stderr, "[DEBUG] stcpp_generate: calling forward_pass with n_tokens=%zu, n_past=%d\n",
            tokens.size(), n_past);
    fflush(stderr);
    if (!stcpp::forward_pass(ggml_ctx, tokens.data(), static_cast<int32_t>(tokens.size()),
                             n_past, logits.data(), error)) {
        if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
            return STCPP_ERROR_CANCELLED;
        }
        return STCPP_ERROR_UNKNOWN;
    }
    n_past = static_cast<int32_t>(tokens.size());

    // Generate tokens
    fprintf(stderr, "[DEBUG] stcpp_generate: starting generation loop, max_tokens=%d, eos_id=%d\n",
            max_tokens, tokenizer->eos_token_id);
    fflush(stderr);
    for (int32_t i = 0; i < max_tokens; ++i) {
        // Check cancel
        if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
            return STCPP_ERROR_CANCELLED;
        }

        // Sample next token
        int32_t next_token = sample_token(logits.data(), model->hparams.n_vocab, params);
        generated_tokens.push_back(next_token);

        fprintf(stderr, "[DEBUG] stcpp_generate: i=%d, next_token=%d\n", i, next_token);
        fflush(stderr);

        // Check for EOS
        if (next_token == tokenizer->eos_token_id) {
            fprintf(stderr, "[DEBUG] stcpp_generate: EOS detected, breaking\n");
            fflush(stderr);
            break;
        }

        // Forward pass for next token
        if (!stcpp::forward_pass(ggml_ctx, &next_token, 1, n_past, logits.data(), error)) {
            if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
                return STCPP_ERROR_CANCELLED;
            }
            return STCPP_ERROR_UNKNOWN;
        }
        n_past++;
    }

    // Detokenize
    fprintf(stderr, "[DEBUG] stcpp_generate: detokenizing %zu tokens\n", generated_tokens.size());
    fflush(stderr);
    std::string result;
    if (!stcpp::detokenize(*tokenizer, generated_tokens, result, error)) {
        fprintf(stderr, "[DEBUG] stcpp_generate: detokenization failed: %s\n", error.c_str());
        fflush(stderr);
        return STCPP_ERROR_UNKNOWN;
    }

    fprintf(stderr, "[DEBUG] stcpp_generate: result='%s', len=%zu\n", result.c_str(), result.size());
    fflush(stderr);

    // Copy to output
    int32_t len = std::min(static_cast<int32_t>(result.size()), max_output_length - 1);
    std::memcpy(output, result.data(), len);
    output[len] = '\0';

    return STCPP_OK;
}

stcpp_error stcpp_generate_stream(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
) {
    // Validate input
    if (ctx == nullptr) {
        return STCPP_ERROR_INVALID_MODEL;
    }
    if (prompt == nullptr || callback == nullptr) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    auto* ctx_impl = reinterpret_cast<stcpp_context_impl*>(ctx);
    if (!ctx_impl->ggml_ctx || !ctx_impl->model) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    stcpp::GgmlContext* ggml_ctx = ctx_impl->ggml_ctx.get();
    stcpp::GgmlModel* model = ctx_impl->model->ggml_model.get();
    stcpp::TokenizerImpl* tokenizer = ctx_impl->model->tokenizer.get();

    if (!model || !tokenizer) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    // Tokenize prompt
    std::vector<int32_t> tokens;
    std::string error;
    if (!stcpp::tokenize(*tokenizer, prompt, tokens, true, error)) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    // Check context size
    if (static_cast<int32_t>(tokens.size()) + max_tokens > ggml_ctx->kv_size) {
        return STCPP_ERROR_OUT_OF_MEMORY;
    }

    // Reset cancel flag
    ggml_ctx->cancel_flag.store(false, std::memory_order_release);

    // Generation loop
    std::vector<float> logits(model->hparams.n_vocab);
    int32_t n_past = 0;

    // Process prompt
    if (!stcpp::forward_pass(ggml_ctx, tokens.data(), static_cast<int32_t>(tokens.size()),
                             n_past, logits.data(), error)) {
        if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
            return STCPP_ERROR_CANCELLED;
        }
        return STCPP_ERROR_UNKNOWN;
    }
    n_past = static_cast<int32_t>(tokens.size());

    // Generate tokens with streaming
    for (int32_t i = 0; i < max_tokens; ++i) {
        // Check cancel
        if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
            return STCPP_ERROR_CANCELLED;
        }

        // Sample next token
        int32_t next_token = sample_token(logits.data(), model->hparams.n_vocab, params);

        // Check for EOS
        if (next_token == tokenizer->eos_token_id) {
            break;
        }

        // Detokenize single token
        std::vector<int32_t> single_token = {next_token};
        std::string token_text;
        if (stcpp::detokenize(*tokenizer, single_token, token_text, error)) {
            // Call streaming callback
            if (!callback(token_text.c_str(), next_token, user_data)) {
                // Callback returned false - stop generation
                return STCPP_OK;
            }
        }

        // Forward pass for next token
        if (!stcpp::forward_pass(ggml_ctx, &next_token, 1, n_past, logits.data(), error)) {
            if (ggml_ctx->cancel_flag.load(std::memory_order_acquire)) {
                return STCPP_ERROR_CANCELLED;
            }
            return STCPP_ERROR_UNKNOWN;
        }
        n_past++;
    }

    return STCPP_OK;
}

void stcpp_cancel(stcpp_context* ctx) {
    if (ctx == nullptr) {
        return;  // Safe to call with null
    }

    // Cast to impl and set cancel flag
    auto* impl = reinterpret_cast<stcpp_context_impl*>(ctx);
    if (impl->ggml_ctx) {
        impl->ggml_ctx->cancel_flag.store(true, std::memory_order_release);
    }
}

stcpp_error stcpp_embeddings(
    stcpp_context* ctx,
    const char* text,
    float* embeddings,
    int32_t max_dims
) {
    if (!ctx || !text || !embeddings) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    auto* ctx_impl = reinterpret_cast<stcpp_context_impl*>(ctx);
    if (!ctx_impl->ggml_ctx || !ctx_impl->model) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    stcpp::GgmlContext* ggml_ctx = ctx_impl->ggml_ctx.get();
    stcpp::GgmlModel* model = ctx_impl->model->ggml_model.get();
    stcpp::TokenizerImpl* tokenizer = ctx_impl->model->tokenizer.get();

    if (!model || !tokenizer) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    // Tokenize text
    std::vector<int32_t> tokens;
    std::string error;
    if (!stcpp::tokenize(*tokenizer, text, tokens, true, error)) {
        return STCPP_ERROR_INVALID_MODEL;
    }

    // For embeddings, we'd run forward pass and extract hidden states
    // This is a simplified implementation
    const int32_t n_embd = model->hparams.n_embd;
    const int32_t dims = std::min(n_embd, max_dims);

    // TODO: Actually compute embeddings from model
    // For now, return zeros as placeholder
    std::memset(embeddings, 0, dims * sizeof(float));

    return STCPP_OK;
}

int32_t stcpp_embeddings_dims(const stcpp_model* model) {
    if (!model) return 0;
    auto* impl = reinterpret_cast<const stcpp_model_impl*>(model);
    return impl->ggml_model ? impl->ggml_model->hparams.n_embd : 0;
}

/* Batch - stub implementations */

stcpp_batch* stcpp_batch_new(stcpp_context* ctx, int32_t max_requests) {
    (void)ctx;
    (void)max_requests;
    return nullptr;
}

void stcpp_batch_free(stcpp_batch* batch) {
    (void)batch;
}

uint64_t stcpp_batch_add(
    stcpp_batch* batch,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
) {
    (void)batch;
    (void)prompt;
    (void)params;
    (void)max_tokens;
    (void)callback;
    (void)user_data;
    return 0;
}

void stcpp_batch_cancel(stcpp_batch* batch, uint64_t request_id) {
    (void)batch;
    (void)request_id;
}

stcpp_error stcpp_batch_decode(stcpp_batch* batch) {
    (void)batch;
    return STCPP_ERROR_UNSUPPORTED_ARCH;
}

int32_t stcpp_batch_n_done(const stcpp_batch* batch) {
    (void)batch;
    return 0;
}

int32_t stcpp_batch_n_active(const stcpp_batch* batch) {
    (void)batch;
    return 0;
}

/* LoRA - stub implementations */

stcpp_lora* stcpp_lora_load(
    stcpp_model* model,
    const char* path,
    float scale
) {
    (void)model;
    (void)path;
    (void)scale;
    return nullptr;
}

void stcpp_lora_free(stcpp_lora* lora) {
    (void)lora;
}

stcpp_error stcpp_lora_apply(stcpp_context* ctx, stcpp_lora* lora) {
    (void)ctx;
    (void)lora;
    return STCPP_ERROR_UNSUPPORTED_ARCH;
}

stcpp_error stcpp_lora_remove(stcpp_context* ctx, stcpp_lora* lora) {
    (void)ctx;
    (void)lora;
    return STCPP_ERROR_UNSUPPORTED_ARCH;
}

/* Prompt cache - stub implementations */

stcpp_error stcpp_prompt_cache_save(
    stcpp_context* ctx,
    const char* prompt,
    const char* cache_path
) {
    (void)ctx;
    (void)prompt;
    (void)cache_path;
    return STCPP_ERROR_UNSUPPORTED_ARCH;
}

stcpp_error stcpp_prompt_cache_load(
    stcpp_context* ctx,
    const char* cache_path
) {
    (void)ctx;
    (void)cache_path;
    return STCPP_ERROR_UNSUPPORTED_ARCH;
}

/* Backend info */

int32_t stcpp_n_backends(void) {
    int32_t count = 0;
#if defined(STCPP_USE_METAL)
    count++;
#endif
#if defined(STCPP_USE_CUDA)
    count++;
#endif
#if defined(STCPP_USE_ROCM)
    count++;
#endif
#if defined(STCPP_USE_VULKAN)
    count++;
#endif
    return count > 0 ? count : 1;  // At least report one backend
}

stcpp_backend_type stcpp_backend_type_at(int32_t index) {
    (void)index;
#if defined(STCPP_USE_METAL)
    return STCPP_BACKEND_METAL;
#elif defined(STCPP_USE_CUDA)
    return STCPP_BACKEND_CUDA;
#elif defined(STCPP_USE_ROCM)
    return STCPP_BACKEND_ROCM;
#elif defined(STCPP_USE_VULKAN)
    return STCPP_BACKEND_VULKAN;
#else
    return STCPP_BACKEND_METAL;
#endif
}

const char* stcpp_backend_name(stcpp_backend_type type) {
    switch (type) {
        case STCPP_BACKEND_METAL:  return "Metal";
        case STCPP_BACKEND_CUDA:   return "CUDA";
        case STCPP_BACKEND_ROCM:   return "ROCm";
        case STCPP_BACKEND_VULKAN: return "Vulkan";
        default: return "Unknown";
    }
}

int32_t stcpp_n_devices(stcpp_backend_type type) {
    (void)type;
    return 1;  // TODO: Query actual device count
}

const char* stcpp_device_name(stcpp_backend_type type, int32_t device_id) {
    (void)type;
    (void)device_id;
    return "Unknown Device";  // TODO: Query actual device name
}

size_t stcpp_device_vram_total(stcpp_backend_type type, int32_t device_id) {
    (void)type;
    (void)device_id;
    return 0;  // TODO: Query actual VRAM
}

size_t stcpp_device_vram_free(stcpp_backend_type type, int32_t device_id) {
    (void)type;
    (void)device_id;
    return 0;  // TODO: Query actual free VRAM
}
