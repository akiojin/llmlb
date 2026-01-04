#pragma once

#include <algorithm>
#include <cstdint>
#include <cstddef>
#include <string>
#include <vector>

#include "core/engine_error.h"

namespace llm_node {

constexpr size_t kDefaultMaxTokens = 2048;

struct ChatMessage {
    std::string role;
    std::string content;
};

using OnTokenCallback = void (*)(void* ctx, uint32_t token_id, uint64_t timestamp_ns);

struct InferenceParams {
    size_t max_tokens{0};
    float temperature{0.8f};
    float top_p{0.9f};
    int top_k{40};
    float repeat_penalty{1.1f};
    uint32_t seed{0};
    std::vector<std::string> stop_sequences;
    OnTokenCallback on_token_callback{nullptr};
    void* on_token_callback_ctx{nullptr};
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

}  // namespace llm_node
