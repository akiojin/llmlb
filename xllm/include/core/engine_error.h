#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef enum allm_error_code {
    XLLM_ERROR_OK = 0,
    XLLM_ERROR_OOM_VRAM = 1,
    XLLM_ERROR_OOM_RAM = 2,
    XLLM_ERROR_MODEL_CORRUPT = 3,
    XLLM_ERROR_TIMEOUT = 4,
    XLLM_ERROR_CANCELLED = 5,
    XLLM_ERROR_UNSUPPORTED = 6,
    XLLM_ERROR_INTERNAL = 7,
    XLLM_ERROR_ABI_MISMATCH = 8,
    XLLM_ERROR_LOAD_FAILED = 9,
} allm_error_code;

#ifdef __cplusplus
}
#endif

#ifdef __cplusplus
namespace xllm {

enum class EngineErrorCode : int {
    kOk = XLLM_ERROR_OK,
    kOomVram = XLLM_ERROR_OOM_VRAM,
    kOomRam = XLLM_ERROR_OOM_RAM,
    kModelCorrupt = XLLM_ERROR_MODEL_CORRUPT,
    kTimeout = XLLM_ERROR_TIMEOUT,
    kCancelled = XLLM_ERROR_CANCELLED,
    kUnsupported = XLLM_ERROR_UNSUPPORTED,
    kInternal = XLLM_ERROR_INTERNAL,
    kAbiMismatch = XLLM_ERROR_ABI_MISMATCH,
    kLoadFailed = XLLM_ERROR_LOAD_FAILED,
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

}  // namespace xllm
#endif
