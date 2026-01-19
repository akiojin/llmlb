#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef enum allm_error_code {
    ALLM_ERROR_OK = 0,
    ALLM_ERROR_OOM_VRAM = 1,
    ALLM_ERROR_OOM_RAM = 2,
    ALLM_ERROR_MODEL_CORRUPT = 3,
    ALLM_ERROR_TIMEOUT = 4,
    ALLM_ERROR_CANCELLED = 5,
    ALLM_ERROR_UNSUPPORTED = 6,
    ALLM_ERROR_INTERNAL = 7,
    ALLM_ERROR_ABI_MISMATCH = 8,
    ALLM_ERROR_LOAD_FAILED = 9,
} allm_error_code;

#ifdef __cplusplus
}
#endif

#ifdef __cplusplus
namespace allm {

enum class EngineErrorCode : int {
    kOk = ALLM_ERROR_OK,
    kOomVram = ALLM_ERROR_OOM_VRAM,
    kOomRam = ALLM_ERROR_OOM_RAM,
    kModelCorrupt = ALLM_ERROR_MODEL_CORRUPT,
    kTimeout = ALLM_ERROR_TIMEOUT,
    kCancelled = ALLM_ERROR_CANCELLED,
    kUnsupported = ALLM_ERROR_UNSUPPORTED,
    kInternal = ALLM_ERROR_INTERNAL,
    kAbiMismatch = ALLM_ERROR_ABI_MISMATCH,
    kLoadFailed = ALLM_ERROR_LOAD_FAILED,
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

}  // namespace allm
#endif
