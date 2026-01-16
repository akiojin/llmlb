/**
 * @file safetensors.h
 * @brief safetensors.cpp - C API for safetensors LLM inference
 *
 * A C++ library for loading and running inference on safetensors format LLMs
 * using ggml backend.
 *
 * @copyright MIT License
 */

#ifndef SAFETENSORS_H
#define SAFETENSORS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Version */
#define STCPP_VERSION_MAJOR 0
#define STCPP_VERSION_MINOR 1
#define STCPP_VERSION_PATCH 0
#define STCPP_ABI_VERSION 1

/* Opaque types */
typedef struct stcpp_model stcpp_model;
typedef struct stcpp_context stcpp_context;
typedef struct stcpp_batch stcpp_batch;
typedef struct stcpp_tokenizer stcpp_tokenizer;
typedef struct stcpp_lora stcpp_lora;

/* Enums */
typedef enum {
    STCPP_BACKEND_METAL  = 0,
    STCPP_BACKEND_CUDA   = 1,
    STCPP_BACKEND_ROCM   = 2,
    STCPP_BACKEND_VULKAN = 3,
    STCPP_BACKEND_CPU    = 4,  // CPU fallback
} stcpp_backend_type;

typedef enum {
    STCPP_LOG_DEBUG = 0,
    STCPP_LOG_INFO  = 1,
    STCPP_LOG_WARN  = 2,
    STCPP_LOG_ERROR = 3,
} stcpp_log_level;

typedef enum {
    STCPP_OK                      =  0,
    STCPP_ERROR_UNKNOWN           = -1,
    STCPP_ERROR_FILE_NOT_FOUND    = -2,
    STCPP_ERROR_INVALID_MODEL     = -3,
    STCPP_ERROR_OUT_OF_MEMORY     = -4,
    STCPP_ERROR_GPU_NOT_FOUND     = -5,
    STCPP_ERROR_UNSUPPORTED_ARCH  = -6,
    STCPP_ERROR_CANCELLED         = -7,
    STCPP_ERROR_VRAM_INSUFFICIENT = -8,
} stcpp_error;

/* KV cache quantization type (Task 59 - VRAM optimization) */
typedef enum {
    STCPP_KV_QUANT_NONE = 0,    /* No quantization (FP16/FP32) */
    STCPP_KV_QUANT_INT8 = 1,    /* INT8 quantization (~50% VRAM) */
    STCPP_KV_QUANT_FP8  = 2,    /* FP8 quantization (~50% VRAM) */
} stcpp_kv_quant_type;

/* Structs */
typedef struct {
    int32_t n_ctx;              /* Context size (default: 4096) */
    int32_t n_batch;            /* Batch size (default: 512) */
    int32_t n_threads;          /* CPU threads (0: auto) */
    int32_t n_gpu_layers;       /* GPU layers (-1: all) */
    int32_t device_id;          /* GPU device ID (default: 0) */
    bool    use_mmap;           /* Use mmap (default: true) */
    bool    use_mlock;          /* Use mlock (default: false) */
    bool    kv_cache_quant;     /* KV cache quantization (default: false) */
    stcpp_kv_quant_type kv_quant_type; /* KV quantization type (Task 59) */
    stcpp_backend_type backend; /* Backend type */
} stcpp_context_params;

typedef struct {
    float   temperature;        /* Temperature (default: 1.0) */
    float   top_p;              /* Top-p (default: 1.0) */
    int32_t top_k;              /* Top-k (default: -1 = disabled) */
    float   min_p;              /* Min-p (default: 0.0) */
    float   repeat_penalty;     /* Repeat penalty (default: 1.0) */
    float   presence_penalty;   /* Presence penalty (default: 0.0) */
    float   frequency_penalty;  /* Frequency penalty (default: 0.0) */
    int32_t seed;               /* Random seed (-1: random) */
} stcpp_sampling_params;

typedef struct {
    size_t vram_required;       /* Required VRAM (bytes) */
    size_t vram_available;      /* Available VRAM (bytes) */
    bool   can_load;            /* Whether model can be loaded */
} stcpp_vram_estimate;

/* Detailed VRAM usage breakdown (Task 59) */
typedef struct {
    size_t weights_bytes;       /* Model weights memory */
    size_t kv_cache_bytes;      /* KV cache memory */
    size_t compute_bytes;       /* Compute buffer memory */
    size_t total_bytes;         /* Total VRAM usage */
    size_t peak_bytes;          /* Peak VRAM usage */
    float  kv_cache_ratio;      /* KV cache as percentage (0.0-1.0) */
} stcpp_vram_usage;

/* Callbacks */
typedef void (*stcpp_log_callback)(
    stcpp_log_level level,
    const char* message,
    void* user_data
);

typedef bool (*stcpp_stream_callback)(
    const char* token_text,
    int32_t token_id,
    void* user_data
);

typedef void (*stcpp_error_callback)(
    stcpp_error error,
    const char* message,
    void* user_data
);

/* Initialization / Cleanup */
void stcpp_init(void);
void stcpp_free(void);
void stcpp_free_string(char* str);  /* Free a string returned by stcpp functions */
const char* stcpp_version(void);
int32_t stcpp_abi_version(void);
void stcpp_set_log_callback(stcpp_log_callback callback, void* user_data);
void stcpp_set_log_level(stcpp_log_level level);

/* Model */
stcpp_model* stcpp_model_load(
    const char* path,
    stcpp_error_callback error_cb,
    void* user_data
);
void stcpp_model_free(stcpp_model* model);
const char* stcpp_model_name(const stcpp_model* model);
int32_t stcpp_model_n_layers(const stcpp_model* model);
int32_t stcpp_model_n_heads(const stcpp_model* model);
int32_t stcpp_model_hidden_size(const stcpp_model* model);
int32_t stcpp_model_vocab_size(const stcpp_model* model);
int32_t stcpp_model_max_context(const stcpp_model* model);

/**
 * @brief Check if model has trained chat tokens (instruct model vs base model)
 * @param model Model handle
 * @return true if model has distinct embeddings for chat tokens (instruct model),
 *         false if chat tokens have identical embeddings (base model)
 * @note Base models should not use chat templates as they produce garbage output
 */
bool stcpp_model_has_trained_chat_tokens(const stcpp_model* model);

stcpp_vram_estimate stcpp_model_estimate_vram(
    const char* path,
    stcpp_backend_type backend,
    int32_t device_id
);

/* Context */
stcpp_context_params stcpp_context_default_params(void);
stcpp_context* stcpp_context_new(
    stcpp_model* model,
    stcpp_context_params params
);
void stcpp_context_free(stcpp_context* ctx);
void stcpp_context_kv_cache_clear(stcpp_context* ctx);

/* VRAM monitoring (Task 59) */
stcpp_vram_usage stcpp_context_vram_usage(const stcpp_context* ctx);
size_t stcpp_context_kv_cache_size(const stcpp_context* ctx);
float stcpp_context_kv_cache_utilization(const stcpp_context* ctx);

/* Tokenizer */
stcpp_tokenizer* stcpp_model_get_tokenizer(stcpp_model* model);
int32_t stcpp_tokenize(
    const stcpp_tokenizer* tokenizer,
    const char* text,
    int32_t* tokens,
    int32_t max_tokens,
    bool add_special
);
int32_t stcpp_detokenize(
    const stcpp_tokenizer* tokenizer,
    const int32_t* tokens,
    int32_t n_tokens,
    char* text,
    int32_t max_length
);
int32_t stcpp_apply_chat_template(
    const stcpp_tokenizer* tokenizer,
    const char* messages_json,
    char* output,
    int32_t max_length,
    bool add_generation_prompt
);
int32_t stcpp_token_bos(const stcpp_tokenizer* tokenizer);
int32_t stcpp_token_eos(const stcpp_tokenizer* tokenizer);
int32_t stcpp_token_pad(const stcpp_tokenizer* tokenizer);

/* Inference */
stcpp_sampling_params stcpp_sampling_default_params(void);
stcpp_error stcpp_generate(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    char* output,
    int32_t max_output_length
);
stcpp_error stcpp_generate_stream(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
);
void stcpp_cancel(stcpp_context* ctx);
stcpp_error stcpp_embeddings(
    stcpp_context* ctx,
    const char* text,
    float* embeddings,
    int32_t max_dims
);
int32_t stcpp_embeddings_dims(const stcpp_model* model);

/* Batch processing */
stcpp_batch* stcpp_batch_new(stcpp_context* ctx, int32_t max_requests);
void stcpp_batch_free(stcpp_batch* batch);
uint64_t stcpp_batch_add(
    stcpp_batch* batch,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
);
void stcpp_batch_cancel(stcpp_batch* batch, uint64_t request_id);
stcpp_error stcpp_batch_decode(stcpp_batch* batch);
int32_t stcpp_batch_n_done(const stcpp_batch* batch);
int32_t stcpp_batch_n_active(const stcpp_batch* batch);

/* LoRA */
stcpp_lora* stcpp_lora_load(
    stcpp_model* model,
    const char* path,
    float scale
);
void stcpp_lora_free(stcpp_lora* lora);
stcpp_error stcpp_lora_apply(stcpp_context* ctx, stcpp_lora* lora);
stcpp_error stcpp_lora_remove(stcpp_context* ctx, stcpp_lora* lora);

/* Prompt cache */
stcpp_error stcpp_prompt_cache_save(
    stcpp_context* ctx,
    const char* prompt,
    const char* cache_path
);
stcpp_error stcpp_prompt_cache_load(
    stcpp_context* ctx,
    const char* cache_path
);

/* Backend info */
int32_t stcpp_n_backends(void);
stcpp_backend_type stcpp_backend_type_at(int32_t index);
const char* stcpp_backend_name(stcpp_backend_type type);
int32_t stcpp_n_devices(stcpp_backend_type type);
const char* stcpp_device_name(stcpp_backend_type type, int32_t device_id);
size_t stcpp_device_vram_total(stcpp_backend_type type, int32_t device_id);
size_t stcpp_device_vram_free(stcpp_backend_type type, int32_t device_id);

#ifdef __cplusplus
}
#endif

#endif /* SAFETENSORS_H */
