#include <gpt-oss/functions.h>
#include <gpt-oss/macros.h>
#include <gpt-oss/types.h>

#include <cstdio>
#include <cstdlib>

namespace {
constexpr const char* kRuntimeName = "nemotron_cuda";

bool should_log() {
    return std::getenv("LLM_NODE_CUDA_RUNTIME_LOG") != nullptr;
}

void log_not_implemented(const char* fn) {
    if (!should_log()) return;
    std::fprintf(stderr, "[%s] %s: CUDA runtime not implemented\n", kRuntimeName, fn);
}

void clear_tokens(uint32_t* tokens_out, size_t* count_out) {
    if (tokens_out) *tokens_out = 0;
    if (count_out) *count_out = 0;
}
}  // namespace

extern "C" {

GPTOSS_ABI enum gptoss_status gptoss_model_create_from_file(
    const char* path,
    gptoss_model_t* model_out) {
    if (model_out) *model_out = nullptr;
    (void)path;
    log_not_implemented("gptoss_model_create_from_file");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_model_get_tokenizer(
    gptoss_model_t model,
    gptoss_tokenizer_t* tokenizer_out) {
    if (tokenizer_out) *tokenizer_out = nullptr;
    (void)model;
    log_not_implemented("gptoss_model_get_tokenizer");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_model_get_max_context_length(
    gptoss_model_t model,
    size_t* max_context_length_out) {
    (void)model;
    (void)max_context_length_out;
    log_not_implemented("gptoss_model_get_max_context_length");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_model_release(
    gptoss_model_t model) {
    (void)model;
    log_not_implemented("gptoss_model_release");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_tokenizer_get_special_token_id(
    gptoss_tokenizer_t tokenizer,
    enum gptoss_special_token token_type,
    uint32_t* token_id_out) {
    if (token_id_out) *token_id_out = 0;
    (void)tokenizer;
    (void)token_type;
    log_not_implemented("gptoss_tokenizer_get_special_token_id");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_tokenizer_get_num_text_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_text_tokens_out) {
    if (num_text_tokens_out) *num_text_tokens_out = 0;
    (void)tokenizer;
    log_not_implemented("gptoss_tokenizer_get_num_text_tokens");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_tokenizer_decode(
    gptoss_tokenizer_t tokenizer,
    uint32_t token_id,
    const void** token_ptr_out,
    size_t* token_size_out) {
    (void)tokenizer;
    (void)token_id;
    if (token_ptr_out) *token_ptr_out = nullptr;
    if (token_size_out) *token_size_out = 0;
    log_not_implemented("gptoss_tokenizer_decode");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_tokenizer_release(
    gptoss_tokenizer_t tokenizer) {
    (void)tokenizer;
    log_not_implemented("gptoss_tokenizer_release");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_create(
    gptoss_model_t model,
    size_t context_length,
    size_t max_batch_tokens,
    gptoss_context_t* context_out) {
    if (context_out) *context_out = nullptr;
    (void)model;
    (void)context_length;
    (void)max_batch_tokens;
    log_not_implemented("gptoss_context_create");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_get_num_tokens(
    gptoss_context_t context,
    size_t* num_tokens_out) {
    if (num_tokens_out) *num_tokens_out = 0;
    (void)context;
    log_not_implemented("gptoss_context_get_num_tokens");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_append_tokens(
    gptoss_context_t context,
    const uint32_t* tokens,
    size_t num_tokens) {
    (void)context;
    (void)tokens;
    (void)num_tokens;
    log_not_implemented("gptoss_context_append_tokens");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_append_chars(
    gptoss_context_t context,
    const char* chars,
    size_t num_chars) {
    (void)context;
    (void)chars;
    (void)num_chars;
    log_not_implemented("gptoss_context_append_chars");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_sample(
    gptoss_context_t context,
    float temperature,
    uint64_t seed,
    size_t max_tokens,
    uint32_t* tokens_out,
    size_t* num_tokens_out) {
    (void)context;
    (void)temperature;
    (void)seed;
    (void)max_tokens;
    clear_tokens(tokens_out, num_tokens_out);
    log_not_implemented("gptoss_context_sample");
    return gptoss_status_unsupported_system;
}

GPTOSS_ABI enum gptoss_status gptoss_context_release(
    gptoss_context_t context) {
    (void)context;
    log_not_implemented("gptoss_context_release");
    return gptoss_status_unsupported_system;
}

}  // extern "C"
