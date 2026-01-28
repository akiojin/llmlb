#pragma once

#include <algorithm>
#include <cstdint>
#include <cstddef>
#include <string>
#include <vector>

#include "core/engine_error.h"

namespace xllm {

constexpr size_t kDefaultMaxTokens = 2048;

struct ChatMessage {
    std::string role;
    std::string content;
};

/// OpenAI-compatible tool/function definition
struct ToolDefinition {
    std::string name;
    std::string description;
    std::string parameters_json;  // JSON schema as string
};

/// Parsed tool call from model output
struct ToolCall {
    std::string id;
    std::string function_name;
    std::string arguments_json;
};

struct LoraRequest {
    std::string name;
    std::string path;
    float scale{1.0f};
};

using OnTokenCallback = void (*)(void* ctx, uint32_t token_id, uint64_t timestamp_ns);
/// T182: Callback to check if generation should abort (returns true to abort)
using AbortCallback = bool (*)(void* ctx);

/// Token-level log probability information for OpenAI API compatibility
struct TokenLogprob {
    std::string token;                                      // The generated token
    double logprob{0.0};                                   // Log probability of the token
    std::vector<std::pair<std::string, double>> top_logprobs;  // Top alternative tokens with logprobs
};

struct InferenceParams {
    size_t max_tokens{0};
    float temperature{0.8f};
    float top_p{0.9f};
    int top_k{40};
    float repeat_penalty{1.1f};
    uint32_t seed{0};
    std::vector<std::string> stop_sequences;
    std::vector<ToolDefinition> tools;  // Function calling tools
    std::string forced_tool_name;       // tool_choiceで指定された関数名（空なら未指定）
    std::string grammar;                // llama.cpp GBNF grammar (empty = no constraint)
    std::string draft_model;            // Speculative decoding: draft model name
    std::string draft_model_path;       // Speculative decoding: resolved draft model path
    int draft_max_tokens{0};            // Speculative decoding: max draft tokens (0 = default)
    int draft_min_tokens{0};            // Speculative decoding: min draft tokens
    std::string chat_template;          // Modelfile chat template (Jinja2)
    std::vector<LoraRequest> loras;     // Dynamic LoRA adapters to apply
    OnTokenCallback on_token_callback{nullptr};
    void* on_token_callback_ctx{nullptr};
    /// T182: Abort callback for inter-token timeout
    AbortCallback abort_callback{nullptr};
    void* abort_callback_ctx{nullptr};

    // OpenAI互換パラメータ
    float presence_penalty{0.0f};   // -2.0 ~ 2.0
    float frequency_penalty{0.0f};  // -2.0 ~ 2.0
    int n{1};                       // 1 ~ 8 (生成する候補数)
    bool logprobs{false};           // logprobs を返すか
    int top_logprobs{0};            // 0 ~ 20 (上位候補数)

    // Output: if non-null and logprobs=true, filled with actual logprob data
    std::vector<TokenLogprob>* out_logprobs{nullptr};
};

inline size_t resolve_effective_max_tokens(size_t requested,
                                           size_t prompt_tokens,
                                           size_t max_context) {
    if (max_context == 0) {
        return requested == 0 ? kDefaultMaxTokens : requested;
    }
    if (prompt_tokens >= max_context) {
        return 0;
    }
    const size_t available = max_context - prompt_tokens;
    if (requested == 0) return available;
    return std::min(requested, available);
}

struct ModelLoadResult {
    bool success{false};
    EngineErrorCode error_code{EngineErrorCode::kLoadFailed};
    std::string error_message;
};

}  // namespace xllm
