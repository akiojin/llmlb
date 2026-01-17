#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef enum llm_node_error_code {
    LLM_NODE_ERROR_OK = 0,
    LLM_NODE_ERROR_OOM_VRAM = 1,
    LLM_NODE_ERROR_OOM_RAM = 2,
    LLM_NODE_ERROR_MODEL_CORRUPT = 3,
    LLM_NODE_ERROR_TIMEOUT = 4,
    LLM_NODE_ERROR_CANCELLED = 5,
    LLM_NODE_ERROR_UNSUPPORTED = 6,
    LLM_NODE_ERROR_INTERNAL = 7,
    LLM_NODE_ERROR_ABI_MISMATCH = 8,
    LLM_NODE_ERROR_LOAD_FAILED = 9,
} llm_node_error_code;

#ifdef __cplusplus
}
#endif

#ifdef __cplusplus
namespace llm_node {

enum class EngineErrorCode : int {
    kOk = LLM_NODE_ERROR_OK,
    kOomVram = LLM_NODE_ERROR_OOM_VRAM,
    kOomRam = LLM_NODE_ERROR_OOM_RAM,
    kModelCorrupt = LLM_NODE_ERROR_MODEL_CORRUPT,
    kTimeout = LLM_NODE_ERROR_TIMEOUT,
    kCancelled = LLM_NODE_ERROR_CANCELLED,
    kUnsupported = LLM_NODE_ERROR_UNSUPPORTED,
    kInternal = LLM_NODE_ERROR_INTERNAL,
    kAbiMismatch = LLM_NODE_ERROR_ABI_MISMATCH,
    kLoadFailed = LLM_NODE_ERROR_LOAD_FAILED,
};

inline const char* to_string(EngineErrorCode code) {
    switch (code) {
        case EngineErrorCode::kOk:
            return "OK";
        case EngineErrorCode::kOomVram:
            return "OOM_VRAM";
        case EngineErrorCode::kOomRam:
            return "OOM_RAM";
        case EngineErrorCode::kModelCorrupt:
            return "MODEL_CORRUPT";
        case EngineErrorCode::kTimeout:
            return "TIMEOUT";
        case EngineErrorCode::kCancelled:
            return "CANCELLED";
        case EngineErrorCode::kUnsupported:
            return "UNSUPPORTED";
        case EngineErrorCode::kInternal:
            return "INTERNAL";
        case EngineErrorCode::kAbiMismatch:
            return "ABI_MISMATCH";
        case EngineErrorCode::kLoadFailed:
            return "LOAD_FAILED";
    }
    return "UNKNOWN";
}

}  // namespace llm_node
#endif
