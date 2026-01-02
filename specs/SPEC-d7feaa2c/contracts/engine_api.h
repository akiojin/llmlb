/**
 * @file engine_api.h
 * @brief LLM-Router Engine Plugin C ABI Definition
 * @version 1.0
 *
 * このヘッダーファイルはエンジンプラグインのC ABIを定義します。
 * すべてのプラグインはこのインターフェースを実装する必要があります。
 */

#ifndef LLM_ROUTER_ENGINE_API_H
#define LLM_ROUTER_ENGINE_API_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ABIバージョン - ホストとプラグインで一致必須 */
#define ENGINE_ABI_VERSION 1

/* エクスポートマクロ */
#ifdef _WIN32
#define ENGINE_API __declspec(dllexport)
#else
#define ENGINE_API __attribute__((visibility("default")))
#endif

/*============================================================================
 * 列挙型定義
 *============================================================================*/

/**
 * @brief エラーコード
 */
typedef enum {
    ERR_OK = 0,           /**< 成功 */
    ERR_OOM_VRAM = 1,     /**< VRAM不足 */
    ERR_OOM_RAM = 2,      /**< RAM不足 */
    ERR_MODEL_CORRUPT = 3,/**< モデルファイル破損 */
    ERR_TIMEOUT = 4,      /**< タイムアウト */
    ERR_CANCELLED = 5,    /**< キャンセル */
    ERR_UNSUPPORTED = 6,  /**< 未サポート機能 */
    ERR_INTERNAL = 7,     /**< 内部エラー */
    ERR_ABI_MISMATCH = 8, /**< ABIバージョン不一致 */
    ERR_LOAD_FAILED = 9   /**< ロード失敗 */
} EngineError;

/**
 * @brief モデル形式
 */
typedef enum {
    FORMAT_GGUF = 0,
    FORMAT_SAFETENSORS = 1,
    FORMAT_UNKNOWN = 255
} ModelFormat;

/**
 * @brief GPUバックエンド
 */
typedef enum {
    GPU_METAL = 0,      /**< Apple Silicon (Metal) */
    GPU_DIRECTML = 1,   /**< Windows (DirectML/D3D12) */
    GPU_CUDA = 2,       /**< NVIDIA (CUDA) - 実験 */
    GPU_CPU = 255       /**< CPUフォールバック（非推奨） */
} GpuBackend;

/*============================================================================
 * 構造体定義
 *============================================================================*/

/**
 * @brief エンジン情報
 */
typedef struct {
    uint32_t abi_version;   /**< ABIバージョン */
    const char* engine_id;  /**< エンジンID */
    const char* version;    /**< バージョン文字列 */
} EngineInfo;

/**
 * @brief エンジン設定
 */
typedef struct {
    GpuBackend gpu_backend;      /**< GPUバックエンド */
    uint64_t vram_limit_bytes;   /**< VRAM割当上限 */
    uint32_t max_batch_size;     /**< 最大バッチサイズ */
    uint32_t context_length;     /**< デフォルトコンテキスト長 */
    void* user_data;             /**< ユーザーデータ（オプション） */
} EngineConfig;

/**
 * @brief モデルロード設定
 */
typedef struct {
    const char* model_path;      /**< モデルディレクトリパス */
    ModelFormat format;          /**< モデル形式 */
    const char* quantization;    /**< 量子化タイプ（例: "Q4_K_M"） */
    uint32_t context_length;     /**< コンテキスト長 */
    int use_mmap;                /**< mmap使用フラグ */
} ModelLoadConfig;

/**
 * @brief トークン結果
 */
typedef struct {
    uint32_t token_id;           /**< トークンID */
    float logprob;               /**< 対数確率 */
    float* top_logprobs;         /**< 上位N件の対数確率 */
    const char** top_tokens;     /**< 上位N件のトークン文字列 */
    size_t top_n;                /**< 上位N件の数 */
} TokenResult;

/**
 * @brief 生成リクエスト
 */
typedef struct {
    const char* prompt;          /**< プロンプト文字列 */
    uint32_t max_tokens;         /**< 最大生成トークン数 */
    float temperature;           /**< 温度 */
    float top_p;                 /**< Top-P */
    uint32_t top_k;              /**< Top-K */
    const char** stop_sequences; /**< 停止シーケンス配列 */
    size_t stop_sequences_len;   /**< 停止シーケンス数 */
    int logprobs;                /**< logprobs返却フラグ */
    uint32_t top_logprobs;       /**< 上位logprobs数 */
    void* user_data;             /**< ユーザーデータ */
} GenerateRequest;

/*============================================================================
 * コールバック型定義
 *============================================================================*/

/**
 * @brief トークン生成コールバック
 * @param ctx ユーザーコンテキスト
 * @param token 生成されたトークン
 * @param timestamp_ns 生成時刻（ナノ秒）
 */
typedef void (*OnTokenCallback)(
    void* ctx,
    const TokenResult* token,
    uint64_t timestamp_ns
);

/**
 * @brief キャンセルチェックコールバック
 * @param ctx ユーザーコンテキスト
 * @return 0: 継続, 1: キャンセル
 */
typedef int (*CheckCancelCallback)(void* ctx);

/*============================================================================
 * エクスポート関数（プラグイン実装必須）
 *============================================================================*/

/**
 * @brief エンジン情報を取得
 * @return エンジン情報へのポインタ
 */
ENGINE_API EngineInfo* engine_get_info(void);

/**
 * @brief エンジンを初期化
 * @param config 設定
 * @return エラーコード
 */
ENGINE_API EngineError engine_init(const EngineConfig* config);

/**
 * @brief モデルをロード
 * @param config ロード設定
 * @return エラーコード
 */
ENGINE_API EngineError engine_load_model(const ModelLoadConfig* config);

/**
 * @brief モデルをアンロード
 * @param model_id モデルID
 * @return エラーコード
 */
ENGINE_API EngineError engine_unload_model(const char* model_id);

/**
 * @brief テキスト生成（ストリーミング）
 * @param model_id モデルID
 * @param request 生成リクエスト
 * @param on_token トークンコールバック
 * @param check_cancel キャンセルチェックコールバック
 * @param ctx コールバックコンテキスト
 * @return エラーコード
 */
ENGINE_API EngineError engine_generate(
    const char* model_id,
    const GenerateRequest* request,
    OnTokenCallback on_token,
    CheckCancelCallback check_cancel,
    void* ctx
);

/**
 * @brief エンジンをシャットダウン
 */
ENGINE_API void engine_shutdown(void);

/**
 * @brief VRAM使用量を取得
 * @return 使用中のVRAMバイト数
 */
ENGINE_API uint64_t engine_get_vram_usage(void);

/**
 * @brief 最後のエラーメッセージを取得
 * @return エラーメッセージ文字列（NULLの場合あり）
 */
ENGINE_API const char* engine_get_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* LLM_ROUTER_ENGINE_API_H */
