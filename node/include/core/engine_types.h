#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace llm_node {

enum class EngineErrorCode : uint32_t {
    kOk = 0,
    kInvalidArgument = 1,
    kNotFound = 2,
    kUnsupported = 3,
    kUnavailable = 4,
    kResourceExhausted = 5,
    kTimeout = 6,
    kCancelled = 7,
    kInternal = 8,
    kUnknown = 9,
};

struct ChatMessage {
    std::string role;
    std::string content;
};

using OnTokenCallback = void (*)(void* ctx, uint32_t token_id, uint64_t timestamp_ns);

struct InferenceParams {
    size_t max_tokens{2048};
    float temperature{0.8f};
    float top_p{0.9f};
    int top_k{40};
    float repeat_penalty{1.1f};
    uint32_t seed{0};
    std::vector<std::string> stop_sequences;
    OnTokenCallback on_token_callback{nullptr};
    void* on_token_callback_ctx{nullptr};
};

struct ModelLoadResult {
    bool success{false};
    EngineErrorCode code{EngineErrorCode::kUnknown};
    std::string error_message;
};

}  // namespace llm_node
