# C API Contract: safetensors.cpp

**機能ID**: SPEC-69549000
**日付**: 2026-01-06
**プレフィックス**: `stcpp_*`

## 概要

safetensors.cppの公開C API仕様。llama.cpp互換のコールバック方式を採用。

## バージョン

```c
#define STCPP_VERSION_MAJOR 0
#define STCPP_VERSION_MINOR 1
#define STCPP_VERSION_PATCH 0
#define STCPP_ABI_VERSION 1
```

## 型定義

### 不透明型

```c
typedef struct stcpp_model stcpp_model;
typedef struct stcpp_context stcpp_context;
typedef struct stcpp_batch stcpp_batch;
typedef struct stcpp_tokenizer stcpp_tokenizer;
typedef struct stcpp_lora stcpp_lora;
```

### 列挙型

```c
typedef enum {
    STCPP_BACKEND_METAL  = 0,
    STCPP_BACKEND_CUDA   = 1,
    STCPP_BACKEND_ROCM   = 2,
    STCPP_BACKEND_VULKAN = 3,
} stcpp_backend_type;

typedef enum {
    STCPP_LOG_DEBUG = 0,
    STCPP_LOG_INFO  = 1,
    STCPP_LOG_WARN  = 2,
    STCPP_LOG_ERROR = 3,
} stcpp_log_level;

typedef enum {
    STCPP_OK                    = 0,
    STCPP_ERROR_UNKNOWN         = -1,
    STCPP_ERROR_FILE_NOT_FOUND  = -2,
    STCPP_ERROR_INVALID_MODEL   = -3,
    STCPP_ERROR_OUT_OF_MEMORY   = -4,
    STCPP_ERROR_GPU_NOT_FOUND   = -5,
    STCPP_ERROR_UNSUPPORTED_ARCH = -6,
    STCPP_ERROR_CANCELLED       = -7,
    STCPP_ERROR_VRAM_INSUFFICIENT = -8,
} stcpp_error;
```

### 構造体

```c
typedef struct {
    int32_t n_ctx;           // コンテキストサイズ（デフォルト: 2048）
    int32_t n_batch;         // バッチサイズ（デフォルト: 512）
    int32_t n_threads;       // CPUスレッド数（デフォルト: 自動）
    int32_t n_gpu_layers;    // GPUにオフロードするレイヤー数（-1: 全て）
    int32_t device_id;       // GPUデバイスID（デフォルト: 0）
    bool    use_mmap;        // mmapを使用（デフォルト: true）
    bool    kv_cache_quant;  // KVキャッシュ量子化（デフォルト: false）
    stcpp_backend_type backend;  // バックエンド
} stcpp_context_params;

typedef struct {
    float   temperature;      // 温度（デフォルト: 1.0）
    float   top_p;           // Top-p（デフォルト: 1.0）
    int32_t top_k;           // Top-k（デフォルト: -1 = 無効）
    float   min_p;           // Min-p（デフォルト: 0.0）
    float   repeat_penalty;  // 繰り返しペナルティ（デフォルト: 1.0）
    float   presence_penalty;  // 存在ペナルティ（デフォルト: 0.0）
    float   frequency_penalty; // 頻度ペナルティ（デフォルト: 0.0）
    int32_t seed;            // 乱数シード（-1: ランダム）
} stcpp_sampling_params;

typedef struct {
    size_t vram_required;    // 必要VRAM（バイト）
    size_t vram_available;   // 利用可能VRAM（バイト）
    bool   can_load;         // ロード可能か
} stcpp_vram_estimate;
```

### コールバック型

```c
// ログコールバック
typedef void (*stcpp_log_callback)(
    stcpp_log_level level,
    const char* message,
    void* user_data
);

// ストリーミングコールバック（トークンごとに呼ばれる）
// 戻り値: true=継続, false=キャンセル
typedef bool (*stcpp_stream_callback)(
    const char* token_text,
    int32_t token_id,
    void* user_data
);

// エラーコールバック
typedef void (*stcpp_error_callback)(
    stcpp_error error,
    const char* message,
    void* user_data
);
```

## 関数

### 初期化・終了

```c
// ライブラリ初期化
void stcpp_init(void);

// ライブラリ終了
void stcpp_free(void);

// バージョン取得
const char* stcpp_version(void);

// ABIバージョン取得
int32_t stcpp_abi_version(void);

// ログコールバック設定
void stcpp_set_log_callback(stcpp_log_callback callback, void* user_data);

// ログレベル設定
void stcpp_set_log_level(stcpp_log_level level);
```

### モデル

```c
// モデルロード
stcpp_model* stcpp_model_load(
    const char* path,           // モデルディレクトリパス
    stcpp_error_callback error_cb,
    void* user_data
);

// モデル解放
void stcpp_model_free(stcpp_model* model);

// モデル情報取得
const char* stcpp_model_name(const stcpp_model* model);
int32_t stcpp_model_n_layers(const stcpp_model* model);
int32_t stcpp_model_n_heads(const stcpp_model* model);
int32_t stcpp_model_hidden_size(const stcpp_model* model);
int32_t stcpp_model_vocab_size(const stcpp_model* model);
int32_t stcpp_model_max_context(const stcpp_model* model);

// VRAM見積もり
stcpp_vram_estimate stcpp_model_estimate_vram(
    const char* path,
    stcpp_backend_type backend,
    int32_t device_id
);
```

### コンテキスト

```c
// デフォルトパラメータ取得
stcpp_context_params stcpp_context_default_params(void);

// コンテキスト作成
stcpp_context* stcpp_context_new(
    stcpp_model* model,
    stcpp_context_params params
);

// コンテキスト解放
void stcpp_context_free(stcpp_context* ctx);

// KVキャッシュクリア
void stcpp_context_kv_cache_clear(stcpp_context* ctx);
```

### トークナイザー

```c
// トークナイザー取得
stcpp_tokenizer* stcpp_model_get_tokenizer(stcpp_model* model);

// トークン化
int32_t stcpp_tokenize(
    const stcpp_tokenizer* tokenizer,
    const char* text,
    int32_t* tokens,        // 出力バッファ
    int32_t max_tokens,     // バッファサイズ
    bool add_special        // 特殊トークン追加
);

// デトークン化
int32_t stcpp_detokenize(
    const stcpp_tokenizer* tokenizer,
    const int32_t* tokens,
    int32_t n_tokens,
    char* text,             // 出力バッファ
    int32_t max_length
);

// チャットテンプレート適用
int32_t stcpp_apply_chat_template(
    const stcpp_tokenizer* tokenizer,
    const char* messages_json,  // JSON形式のメッセージ配列
    char* output,               // 出力バッファ
    int32_t max_length,
    bool add_generation_prompt
);

// 特殊トークンID取得
int32_t stcpp_token_bos(const stcpp_tokenizer* tokenizer);
int32_t stcpp_token_eos(const stcpp_tokenizer* tokenizer);
int32_t stcpp_token_pad(const stcpp_tokenizer* tokenizer);
```

### 推論

```c
// デフォルトサンプリングパラメータ取得
stcpp_sampling_params stcpp_sampling_default_params(void);

// テキスト生成（同期）
stcpp_error stcpp_generate(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    char* output,           // 出力バッファ
    int32_t max_output_length
);

// テキスト生成（ストリーミング）
stcpp_error stcpp_generate_stream(
    stcpp_context* ctx,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
);

// 推論キャンセル
void stcpp_cancel(stcpp_context* ctx);

// 埋め込み生成
stcpp_error stcpp_embeddings(
    stcpp_context* ctx,
    const char* text,
    float* embeddings,      // 出力バッファ
    int32_t max_dims
);

// 埋め込み次元数取得
int32_t stcpp_embeddings_dims(const stcpp_model* model);
```

### バッチ処理

```c
// バッチ作成
stcpp_batch* stcpp_batch_new(stcpp_context* ctx, int32_t max_requests);

// バッチ解放
void stcpp_batch_free(stcpp_batch* batch);

// リクエスト追加
uint64_t stcpp_batch_add(
    stcpp_batch* batch,
    const char* prompt,
    stcpp_sampling_params params,
    int32_t max_tokens,
    stcpp_stream_callback callback,
    void* user_data
);

// リクエストキャンセル
void stcpp_batch_cancel(stcpp_batch* batch, uint64_t request_id);

// バッチ推論実行（1ステップ）
stcpp_error stcpp_batch_decode(stcpp_batch* batch);

// 完了リクエスト数取得
int32_t stcpp_batch_n_done(const stcpp_batch* batch);

// アクティブリクエスト数取得
int32_t stcpp_batch_n_active(const stcpp_batch* batch);
```

### LoRA

```c
// LoRAロード
stcpp_lora* stcpp_lora_load(
    stcpp_model* model,
    const char* path,
    float scale             // スケール係数（デフォルト: 1.0）
);

// LoRA解放
void stcpp_lora_free(stcpp_lora* lora);

// LoRA適用（ホットリロード）
stcpp_error stcpp_lora_apply(
    stcpp_context* ctx,
    stcpp_lora* lora
);

// LoRA解除
stcpp_error stcpp_lora_remove(
    stcpp_context* ctx,
    stcpp_lora* lora
);
```

### プロンプトキャッシュ

```c
// プロンプトキャッシュ保存
stcpp_error stcpp_prompt_cache_save(
    stcpp_context* ctx,
    const char* prompt,
    const char* cache_path
);

// プロンプトキャッシュ読み込み
stcpp_error stcpp_prompt_cache_load(
    stcpp_context* ctx,
    const char* cache_path
);
```

### バックエンド情報

```c
// 利用可能なバックエンド数
int32_t stcpp_n_backends(void);

// バックエンドタイプ取得
stcpp_backend_type stcpp_backend_type_at(int32_t index);

// バックエンド名取得
const char* stcpp_backend_name(stcpp_backend_type type);

// デバイス数取得
int32_t stcpp_n_devices(stcpp_backend_type type);

// デバイス名取得
const char* stcpp_device_name(stcpp_backend_type type, int32_t device_id);

// VRAM情報取得
size_t stcpp_device_vram_total(stcpp_backend_type type, int32_t device_id);
size_t stcpp_device_vram_free(stcpp_backend_type type, int32_t device_id);
```

## 使用例

### 基本的なテキスト生成

```c
#include "safetensors.h"

int main() {
    stcpp_init();

    // モデルロード
    stcpp_model* model = stcpp_model_load("./model", NULL, NULL);
    if (!model) {
        fprintf(stderr, "Failed to load model\n");
        return 1;
    }

    // コンテキスト作成
    stcpp_context_params params = stcpp_context_default_params();
    params.n_ctx = 4096;
    stcpp_context* ctx = stcpp_context_new(model, params);

    // 生成
    char output[4096];
    stcpp_sampling_params sampling = stcpp_sampling_default_params();
    sampling.temperature = 0.7f;

    stcpp_error err = stcpp_generate(
        ctx, "Hello, ", sampling, 100, output, sizeof(output)
    );

    if (err == STCPP_OK) {
        printf("%s\n", output);
    }

    // 解放
    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();

    return 0;
}
```

### ストリーミング生成

```c
bool on_token(const char* token, int32_t id, void* user_data) {
    printf("%s", token);
    fflush(stdout);
    return true;  // 継続
}

int main() {
    // ... 初期化 ...

    stcpp_generate_stream(
        ctx, "Once upon a time", sampling, 200, on_token, NULL
    );

    // ... 解放 ...
}
```

## スレッドセーフティ

- 全ての関数はスレッドセーフ
- 同一`stcpp_context`への並行アクセスは内部でシリアライズされる
- 異なる`stcpp_context`は完全に独立して並行実行可能
- `stcpp_model`は複数の`stcpp_context`で共有可能（読み取り専用）
