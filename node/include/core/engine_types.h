#pragma once

#include <algorithm>
#include <cstdint>
#include <cstddef>
#include <string>
#include <vector>

namespace llm_node {

constexpr size_t kDefaultMaxTokens = 2048;

struct ChatMessage {
    std::string role;
    std::string content;
};

struct InferenceParams {
    size_t max_tokens{kDefaultMaxTokens};
    float temperature{0.8f};
    float top_p{0.9f};
    int top_k{40};
    float repeat_penalty{1.1f};
    uint32_t seed{0};
};

inline size_t resolve_effective_max_tokens(size_t requested,
                                           size_t prompt_tokens,
                                           size_t max_context) {
    if (max_context == 0 || prompt_tokens >= max_context) {
        return requested;
    }
    const size_t available = max_context - prompt_tokens;
    if (requested == 0 || requested == kDefaultMaxTokens) {
        return available;
    }
    return std::min(requested, available);
}

struct ModelLoadResult {
    bool success{false};
    std::string error_message;
};

}  // namespace llm_node
