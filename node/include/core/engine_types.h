#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace llm_node {

struct ChatMessage {
    std::string role;
    std::string content;
};

struct InferenceParams {
    size_t max_tokens{2048};
    float temperature{0.8f};
    float top_p{0.9f};
    int top_k{40};
    float repeat_penalty{1.1f};
    uint32_t seed{0};
};

struct ModelLoadResult {
    bool success{false};
    std::string error_message;
};

}  // namespace llm_node
