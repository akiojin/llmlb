# safetensors.cpp API Reference

## Overview

safetensors.cpp provides a C API for loading and running safetensors format
models with GPU acceleration via ggml backends.

## Initialization

### stcpp_init

```c
stcpp_error stcpp_init(void);
```

Initialize the safetensors.cpp library. Must be called before any other
functions.

**Returns:** `STCPP_OK` on success, error code otherwise.

### stcpp_free

```c
void stcpp_free(void);
```

Release all resources allocated by the library. Call when done using the
library.

## Version Information

### stcpp_version

```c
const char* stcpp_version(void);
```

**Returns:** Version string (e.g., "0.1.0").

### stcpp_abi_version

```c
int32_t stcpp_abi_version(void);
```

**Returns:** ABI version number for compatibility checking.

## Model Operations

### stcpp_model_load

```c
stcpp_model* stcpp_model_load(const char* model_path);
```

Load a model from a directory containing:

- `model.safetensors` or `model-*-of-*.safetensors` (weights)
- `config.json` (model configuration)
- `tokenizer.json` (tokenizer configuration)

**Parameters:**

- `model_path`: Path to the model directory

**Returns:** Pointer to loaded model, or `NULL` on failure.

### stcpp_model_free

```c
void stcpp_model_free(stcpp_model* model);
```

Free a loaded model and its resources.

### stcpp_model_n_ctx

```c
int32_t stcpp_model_n_ctx(const stcpp_model* model);
```

**Returns:** Maximum context length supported by the model.

### stcpp_model_n_vocab

```c
int32_t stcpp_model_n_vocab(const stcpp_model* model);
```

**Returns:** Vocabulary size.

### stcpp_model_vram_estimate

```c
int64_t stcpp_model_vram_estimate(const stcpp_model* model);
```

**Returns:** Estimated VRAM usage in bytes.

## Context Management

### stcpp_context_new

```c
stcpp_context* stcpp_context_new(stcpp_model* model, stcpp_context_params params);
```

Create a new inference context for the given model.

**Parameters:**

- `model`: Loaded model
- `params`: Context parameters (see `stcpp_context_default_params`)

**Returns:** Pointer to context, or `NULL` on failure.

### stcpp_context_free

```c
void stcpp_context_free(stcpp_context* ctx);
```

Free a context and its resources.

### stcpp_context_default_params

```c
stcpp_context_params stcpp_context_default_params(void);
```

Get default context parameters:

```c
typedef struct {
    int32_t n_ctx;           // Context size (default: 2048)
    int32_t n_batch;         // Batch size (default: 512)
    int32_t n_threads;       // CPU threads (default: 4)
    bool use_mmap;           // Memory-mapped files (default: true)
    bool use_mlock;          // Lock memory (default: false)
    int32_t gpu_layers;      // Layers to offload to GPU (default: 0)
    int32_t kv_cache_type;   // KV cache quantization (default: 0 = FP16)
} stcpp_context_params;
```

## Tokenization

### stcpp_tokenize

```c
int32_t stcpp_tokenize(
    const stcpp_model* model,
    const char* text,
    int32_t* tokens,
    int32_t max_tokens,
    bool add_special
);
```

Convert text to tokens.

**Parameters:**

- `model`: Loaded model with tokenizer
- `text`: Input text (UTF-8)
- `tokens`: Output token array
- `max_tokens`: Maximum tokens to write
- `add_special`: Add BOS/EOS tokens

**Returns:** Number of tokens, or negative on error.

### stcpp_detokenize

```c
int32_t stcpp_detokenize(
    const stcpp_model* model,
    const int32_t* tokens,
    int32_t n_tokens,
    char* text,
    int32_t max_chars
);
```

Convert tokens to text.

**Returns:** Number of characters written, or negative on error.

## Text Generation

### stcpp_generate

```c
stcpp_error stcpp_generate(
    stcpp_context* ctx,
    const char* prompt,
    char* output,
    int32_t max_tokens
);
```

Generate text (blocking).

**Parameters:**

- `ctx`: Inference context
- `prompt`: Input prompt
- `output`: Output buffer
- `max_tokens`: Maximum tokens to generate

**Returns:** `STCPP_OK` on success.

### stcpp_generate_stream

```c
stcpp_error stcpp_generate_stream(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_stream_callback callback,
    void* user_data,
    stcpp_sampling_params params,
    int32_t max_tokens
);
```

Generate text with streaming callback.

**Callback signature:**

```c
typedef bool (*stcpp_stream_callback)(const char* token, void* user_data);
```

Return `false` from callback to stop generation.

### stcpp_sampling_default_params

```c
stcpp_sampling_params stcpp_sampling_default_params(void);
```

Get default sampling parameters:

```c
typedef struct {
    float temperature;       // Default: 0.8
    float top_p;            // Default: 0.95
    int32_t top_k;          // Default: 40
    float min_p;            // Default: 0.05
    float repeat_penalty;   // Default: 1.1
    float presence_penalty; // Default: 0.0
    float frequency_penalty;// Default: 0.0
    int32_t seed;           // Default: -1 (random)
} stcpp_sampling_params;
```

### stcpp_cancel

```c
void stcpp_cancel(stcpp_context* ctx);
```

Cancel ongoing generation. Thread-safe.

## Embeddings

### stcpp_embeddings

```c
stcpp_error stcpp_embeddings(
    stcpp_context* ctx,
    const char* text,
    float* embeddings,
    int32_t max_dims
);
```

Generate embeddings for text.

**Returns:** `STCPP_OK` on success.

### stcpp_embeddings_dims

```c
int32_t stcpp_embeddings_dims(const stcpp_model* model);
```

**Returns:** Embedding dimension size.

## Batch Processing

### stcpp_batch_new

```c
stcpp_batch* stcpp_batch_new(stcpp_context* ctx, int32_t max_requests);
```

Create a batch for continuous batching.

### stcpp_batch_free

```c
void stcpp_batch_free(stcpp_batch* batch);
```

### stcpp_batch_add

```c
uint64_t stcpp_batch_add(
    stcpp_batch* batch,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
);
```

Add a request to the batch.

**Returns:** Request ID (>0), or 0 on failure.

### stcpp_batch_cancel

```c
void stcpp_batch_cancel(stcpp_batch* batch, uint64_t request_id);
```

Cancel a specific request in the batch.

### stcpp_batch_decode

```c
stcpp_error stcpp_batch_decode(stcpp_batch* batch);
```

Process one decode step for all active requests.

### stcpp_batch_n_done / stcpp_batch_n_active

```c
int32_t stcpp_batch_n_done(const stcpp_batch* batch);
int32_t stcpp_batch_n_active(const stcpp_batch* batch);
```

Get count of completed/active requests.

## KV Cache

### stcpp_kv_cache_clear

```c
void stcpp_kv_cache_clear(stcpp_context* ctx);
```

Clear the KV cache.

### stcpp_kv_cache_defrag

```c
void stcpp_kv_cache_defrag(stcpp_context* ctx);
```

Defragment the KV cache.

## Prompt Caching

### stcpp_prompt_cache_save

```c
stcpp_error stcpp_prompt_cache_save(
    stcpp_context* ctx,
    const char* path
);
```

Save prompt cache to file.

### stcpp_prompt_cache_load

```c
stcpp_error stcpp_prompt_cache_load(
    stcpp_context* ctx,
    const char* path
);
```

Load prompt cache from file.

## LoRA Adapters

### stcpp_lora_load

```c
stcpp_lora* stcpp_lora_load(
    stcpp_model* model,
    const char* lora_path,
    float scale
);
```

Load a LoRA adapter.

**Parameters:**

- `model`: Base model
- `lora_path`: Path to LoRA safetensors file
- `scale`: LoRA scale factor (typically 1.0)

### stcpp_lora_free

```c
void stcpp_lora_free(stcpp_lora* lora);
```

### stcpp_lora_apply

```c
stcpp_error stcpp_lora_apply(stcpp_context* ctx, stcpp_lora* lora);
```

Apply LoRA to context.

### stcpp_lora_remove

```c
stcpp_error stcpp_lora_remove(stcpp_context* ctx, stcpp_lora* lora);
```

Remove LoRA from context.

## Error Handling

### Error Codes

```c
typedef enum {
    STCPP_OK = 0,
    STCPP_ERROR_INVALID_ARGUMENT = 1,
    STCPP_ERROR_OUT_OF_MEMORY = 2,
    STCPP_ERROR_IO = 3,
    STCPP_ERROR_INVALID_MODEL = 4,
    STCPP_ERROR_NOT_INITIALIZED = 5,
    STCPP_ERROR_CANCELLED = 6,
} stcpp_error;
```

### stcpp_error_string

```c
const char* stcpp_error_string(stcpp_error error);
```

Get human-readable error message.

## Memory Management

### stcpp_free_string

```c
void stcpp_free_string(char* str);
```

Free strings returned by the library.

## Thread Safety

- `stcpp_cancel()` is thread-safe
- Context operations should be serialized per context
- Multiple contexts can run in parallel
- Model loading is thread-safe

## GPU Backends

Supported backends (compile-time options):

- **Metal** (macOS): `STCPP_METAL=ON`
- **CUDA** (NVIDIA): `STCPP_CUDA=ON`
- **ROCm** (AMD): `STCPP_ROCM=ON`
- **Vulkan**: `STCPP_VULKAN=ON`

## Example Usage

```c
#include "safetensors.h"

int main() {
    // Initialize
    stcpp_init();

    // Load model
    stcpp_model* model = stcpp_model_load("./my-model");
    if (!model) {
        fprintf(stderr, "Failed to load model\n");
        return 1;
    }

    // Create context
    stcpp_context_params params = stcpp_context_default_params();
    params.gpu_layers = 32;  // Offload to GPU
    stcpp_context* ctx = stcpp_context_new(model, params);

    // Generate text
    char output[4096];
    stcpp_generate(ctx, "Hello, how are you?", output, 100);
    printf("%s\n", output);

    // Cleanup
    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();

    return 0;
}
```
