# データモデル: LLM-Router独自モデルストレージ

## エンティティ定義

### Node側（C++）

```cpp
// node/include/models/model_storage.h

/// モデルアーティファクトの形式
enum class ModelFormat {
    Gguf,        // llama.cpp互換 (.gguf)
    Safetensors, // HuggingFace形式 (.safetensors)
    Metal,       // Apple Silicon最適化 (.metal.bin)
    Unknown
};

/// モデルアーティファクト情報
struct ModelArtifact {
    std::filesystem::path path;      // ファイルパス
    ModelFormat format;              // 形式
    uint64_t size_bytes;             // ファイルサイズ
    std::string sha256;              // ハッシュ（オプション）
};

/// モデルメタデータ
struct ModelMetadata {
    std::string id;                  // モデルID
    std::string name;                // 表示名（オプション）
    std::string repo;                // HuggingFaceリポジトリ（オプション）
    std::vector<ModelArtifact> artifacts;
};

/// モデルストレージ管理クラス
class ModelStorage {
public:
    explicit ModelStorage(std::filesystem::path models_dir);

    /// 利用可能なモデル一覧を取得
    std::vector<std::string> list_available() const;

    /// モデルパスを解決（ローカル優先）
    std::optional<std::filesystem::path> resolve(const std::string& model_id) const;

    /// モデルをダウンロード（HFから直接取得）
    bool download(const std::string& model_id, const std::string& repo_id);

    /// モデル形式を検出
    ModelFormat detect_format(const std::string& model_id) const;

private:
    std::filesystem::path models_dir_;
};
```

### Router側（Rust）

```rust
// router/src/models/storage.rs

use serde::{Deserialize, Serialize};

/// モデルアーティファクトの形式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Gguf,
    Safetensors,
    Metal,
    Unknown,
}

/// マニフェスト内のファイル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub filename: String,
    pub format: ModelFormat,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// モデルマニフェスト（Node同期用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_id: String,
    pub repo: Option<String>,
    pub files: Vec<ManifestFile>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 登録済みモデル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredModel {
    pub id: String,
    pub name: Option<String>,
    pub repo: Option<String>,
    pub format: ModelFormat,
    pub size_bytes: Option<u64>,
    pub registered_at: chrono::DateTime<chrono::Utc>,
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|-----------------|
| `model_id` | 空でない | "Model ID is required" |
| `model_id` | `..` を含まない | "Invalid model ID: path traversal" |
| `model_id` | `\0` を含まない | "Invalid model ID: null character" |
| `model_id` | 256文字以下 | "Model ID too long" |
| `repo` | HuggingFace形式（org/name） | "Invalid repo format" |
| `size_bytes` | 0より大きい | "Invalid file size" |
| `sha256` | 64文字の16進数 | "Invalid SHA256 hash" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                        Router                                │
│  ┌─────────────────┐    ┌──────────────────┐                │
│  │ RegisteredModel │───→│  ModelManifest   │                │
│  │ (SQLite)        │    │  (API Response)  │                │
│  └─────────────────┘    └──────────────────┘                │
│          │                       │                           │
│          │ /v1/models            │ /v0/models/registry/      │
│          ↓                       │    /{model}/manifest.json │
└──────────┼───────────────────────┼──────────────────────────┘
           │                       │
           │ OpenAI互換API         │ Node同期API
           ↓                       ↓
┌──────────────────────────────────────────────────────────────┐
│                         Node                                  │
│  ┌─────────────────┐    ┌──────────────────┐                 │
│  │  ModelStorage   │───→│  ModelArtifact   │                 │
│  │  (FileSystem)   │    │  (gguf/safetens) │                 │
│  └─────────────────┘    └──────────────────┘                 │
│          │                                                    │
│          ↓                                                    │
│  ~/.llm-router/models/<model_id>/                            │
│      ├── model.gguf                                          │
│      ├── model.safetensors                                   │
│      └── metadata.json (optional)                            │
└──────────────────────────────────────────────────────────────┘
```

## ディレクトリ構造

```text
~/.llm-router/
├── config.json                    # グローバル設定
├── router.db                      # ルーターDB（SQLite）
└── models/                        # モデルストレージ
    ├── llama-3.2-1b/
    │   ├── model.gguf             # GGUFモデル
    │   └── metadata.json          # メタデータ（オプション）
    ├── qwen2.5-coder-7b/
    │   ├── config.json
    │   ├── tokenizer.json
    │   ├── model.safetensors.index.json
    │   └── model-00001-of-00003.safetensors
    └── openai/
        └── gpt-oss-20b/           # 階層形式モデルID
            ├── model.safetensors.index.json
            └── model-*.safetensors
```

## API レスポンス形式

### GET /v0/models/registry/{model_id}/manifest.json

```json
{
  "model_id": "llama-3.2-1b",
  "repo": "bartowski/Llama-3.2-1B-Instruct-GGUF",
  "files": [
    {
      "filename": "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
      "format": "gguf",
      "size_bytes": 800000000,
      "sha256": "abc123..."
    }
  ],
  "created_at": "2024-12-01T00:00:00Z"
}
```
