# データモデル: Nodeエンジンローダー抽象化

## エンティティ定義

### C ABI エクスポート構造体

```c
// engine_api.h

#define ENGINE_ABI_VERSION 1

/// エンジン情報（プラグイン識別）
typedef struct {
    uint32_t abi_version;      // ABIバージョン（互換性チェック用）
    const char* engine_id;     // エンジンID（例: "llama_cpp"）
    const char* version;       // エンジンバージョン（例: "1.0.0"）
} EngineInfo;

/// モデル形式
typedef enum {
    FORMAT_GGUF = 0,
    FORMAT_SAFETENSORS = 1,
    FORMAT_UNKNOWN = 255
} ModelFormat;

/// GPUバックエンド
typedef enum {
    GPU_METAL = 0,      // Apple Silicon
    GPU_DIRECTML = 1,   // Windows DirectX 12
    GPU_CUDA = 2,       // NVIDIA (実験)
    GPU_CPU = 255       // フォールバック（非推奨）
} GpuBackend;

/// エンジン設定
typedef struct {
    GpuBackend gpu_backend;
    uint64_t vram_limit_bytes;   // VRAM割当上限
    uint32_t max_batch_size;     // 最大バッチサイズ
    uint32_t context_length;     // デフォルトコンテキスト長
} EngineConfig;

/// トークン生成結果
typedef struct {
    uint32_t token_id;           // トークンID
    float logprob;               // 対数確率
    float* top_logprobs;         // 上位N件の対数確率（オプション）
    const char** top_tokens;     // 上位N件のトークン（オプション）
    size_t top_n;                // 上位N件の数
} TokenResult;

/// 生成コールバック
typedef void (*OnTokenCallback)(
    void* ctx,
    const TokenResult* token,
    uint64_t timestamp_ns
);

/// エラーコード
typedef enum {
    ERR_OK = 0,
    ERR_OOM_VRAM = 1,
    ERR_OOM_RAM = 2,
    ERR_MODEL_CORRUPT = 3,
    ERR_TIMEOUT = 4,
    ERR_CANCELLED = 5,
    ERR_UNSUPPORTED = 6,
    ERR_INTERNAL = 7,
    ERR_ABI_MISMATCH = 8,
    ERR_LOAD_FAILED = 9
} EngineError;
```

### manifest.json スキーマ

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["id", "version", "abi_version", "gpu_backend", "binary"],
  "properties": {
    "id": {
      "type": "string",
      "description": "エンジンID（一意識別子）"
    },
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$",
      "description": "セマンティックバージョン"
    },
    "abi_version": {
      "type": "string",
      "pattern": "^\\d+$",
      "description": "ABIバージョン"
    },
    "gpu_backend": {
      "type": "string",
      "enum": ["metal", "directml", "cuda"],
      "description": "対応GPUバックエンド"
    },
    "architectures": {
      "type": "array",
      "items": { "type": "string" },
      "description": "サポートするモデルアーキテクチャ"
    },
    "formats": {
      "type": "array",
      "items": { "type": "string", "enum": ["gguf", "safetensors"] },
      "description": "サポートするモデル形式"
    },
    "binary": {
      "type": "string",
      "description": "共有ライブラリファイル名"
    },
    "modalities": {
      "type": "array",
      "items": { "type": "string", "enum": ["completion", "embedding"] },
      "description": "サポートするモダリティ"
    },
    "supports_vision": {
      "type": "boolean",
      "description": "Vision対応可否"
    },
    "license": {
      "type": "string",
      "description": "ライセンス情報"
    }
  }
}
```

### C++ Host側データモデル

```cpp
// engine_host.h

/// プラグインマニフェスト（manifest.json解析結果）
struct PluginManifest {
    std::string id;
    std::string version;
    uint32_t abi_version;
    std::string gpu_backend;
    std::vector<std::string> architectures;
    std::vector<std::string> formats;
    std::string binary;
    std::vector<std::string> modalities;
    bool supports_vision;
    std::string license;
};

/// ロード済みプラグイン
struct LoadedPlugin {
    void* handle;                    // dlopen ハンドル
    PluginManifest manifest;
    std::chrono::steady_clock::time_point loaded_at;
    std::atomic<uint64_t> request_count;
    std::atomic<uint64_t> active_requests;
};

/// モデル記述子（推論に必要な最小情報）
struct ModelDescriptor {
    std::string model_id;
    std::string model_dir;           // モデルファイルディレクトリ
    ModelFormat format;              // GGUF or SafeTensors
    std::string runtime;             // エンジンID（例: "llama_cpp"）
    std::string primary_file;        // 主ファイル（.ggufまたは.index.json）
    std::string quantization;        // 量子化タイプ（例: "Q4_K_M"）
    std::optional<std::string> mmproj; // Vision用mmproj（オプション）
};

/// エンジンレジストリ（runtime → plugin 解決）
class EngineRegistry {
public:
    void register_plugin(const PluginManifest& manifest);
    LoadedPlugin* resolve(const std::string& runtime);
    std::vector<std::string> list_supported_architectures();

private:
    std::map<std::string, LoadedPlugin> plugins_;
};
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|-----------------|
| `abi_version` | ホストABIと一致 | "ABI version mismatch: expected X, got Y" |
| `id` | 既存プラグインと重複不可 | "Plugin ID conflict: X already loaded" |
| `binary` | ファイルが存在 | "Binary not found: path" |
| `architectures` | 空でない | "No architectures specified" |
| `gpu_backend` | 現在のGPUと一致 | "GPU backend mismatch" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                       Node (C++)                            │
│                                                             │
│  ┌─────────────────┐    ┌──────────────────┐               │
│  │  ModelStorage   │───→│ ModelDescriptor  │               │
│  │  (SPEC-dcaeaec4)│    │ (format, runtime)│               │
│  └─────────────────┘    └────────┬─────────┘               │
│                                  │                          │
│                                  ▼                          │
│  ┌─────────────────┐    ┌──────────────────┐               │
│  │ EngineRegistry  │───→│  LoadedPlugin    │               │
│  │ (runtime解決)   │    │  (dll/dylib/so)  │               │
│  └─────────────────┘    └────────┬─────────┘               │
│                                  │                          │
│                                  ▼                          │
│  ┌─────────────────┐    ┌──────────────────┐               │
│  │   EngineHost    │───→│   Engine ABI     │               │
│  │ (プラグイン管理)│    │  (C関数呼び出し) │               │
│  └─────────────────┘    └────────┬─────────┘               │
│                                  │                          │
│                                  ▼                          │
│  ┌──────────────────────────────────────────┐              │
│  │            Engine Plugins (複数)          │              │
│  │  ┌──────────┐  ┌───────────┐  ┌───────┐ │              │
│  │  │llama.cpp │  │ nemotron  │  │ (TBD) │ │              │
│  │  │  (GGUF)  │  │(safetens) │  │       │ │              │
│  │  └──────────┘  └───────────┘  └───────┘ │              │
│  └──────────────────────────────────────────┘              │
└─────────────────────────────────────────────────────────────┘
```

## ディレクトリ構造

```text
engines/
├── llama_cpp/
│   ├── metal/
│   │   ├── manifest.json
│   │   └── libllama_engine.dylib
│   └── directml/
│       ├── manifest.json
│       └── llama_engine.dll
└── nemotron/
    ├── metal/
    │   ├── manifest.json
    │   └── libnemotron_engine.dylib
    └── directml/
        ├── manifest.json
        └── nemotron_engine.dll
```

## エラーコード一覧

| コード | 名前 | 説明 | HTTPステータス |
|--------|------|------|---------------|
| 0 | OK | 成功 | 200 |
| 1 | OOM_VRAM | VRAM不足 | 507 |
| 2 | OOM_RAM | RAM不足 | 507 |
| 3 | MODEL_CORRUPT | モデルファイル破損 | 500 |
| 4 | TIMEOUT | タイムアウト | 504 |
| 5 | CANCELLED | キャンセル | 499 |
| 6 | UNSUPPORTED | 未サポート機能 | 400 |
| 7 | INTERNAL | 内部エラー | 500 |
| 8 | ABI_MISMATCH | ABIバージョン不一致 | 500 |
| 9 | LOAD_FAILED | ロード失敗 | 500 |
