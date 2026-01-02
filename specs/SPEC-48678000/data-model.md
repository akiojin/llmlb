# データモデル: モデル自動解決機能

## エンティティ定義

### SupportedModel（サポートモデル定義）

`supported_models.json`に定義されるモデルエントリ。

```rust
/// サポートモデル定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedModel {
    /// モデル識別子（例: "llama-3.2-1b"）
    pub id: String,
    /// 表示名
    pub name: String,
    /// モデルソース（例: "hf_gguf", "hf_safetensors"）
    pub source: ModelSource,
    /// HuggingFace リポジトリID（例: "meta-llama/Llama-3.2-1B"）
    pub repo_id: Option<String>,
    /// 必要なアーティファクト一覧
    pub artifacts: Vec<Artifact>,
    /// 必要VRAM（MB）
    pub required_vram_mb: u64,
    /// タグ（例: ["chat", "instruct"]）
    pub tags: Vec<String>,
}

/// モデルソース種別
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// HuggingFace GGUF形式
    HfGguf,
    /// HuggingFace safetensors形式
    HfSafetensors,
    /// HuggingFace ONNX形式
    HfOnnx,
    /// 事前定義（ローカル）
    Predefined,
}

/// アーティファクト定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// ファイル名（例: "model.gguf"）
    pub filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: Option<u64>,
    /// SHA256ハッシュ（整合性検証用）
    pub sha256: Option<String>,
}
```

### ModelResolutionResult（解決結果）

```rust
/// モデル解決結果
#[derive(Debug, Clone)]
pub enum ModelResolutionResult {
    /// ローカルに存在
    Local {
        path: PathBuf,
    },
    /// ダウンロード中
    Downloading {
        progress: f32,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    /// ダウンロード完了
    Downloaded {
        path: PathBuf,
    },
    /// エラー
    Error {
        message: String,
        error_type: ResolutionErrorType,
    },
}

/// 解決エラー種別
#[derive(Debug, Clone)]
pub enum ResolutionErrorType {
    /// モデル未定義
    UnsupportedModel,
    /// ダウンロード失敗
    DownloadFailed,
    /// ディスク容量不足
    InsufficientDiskSpace,
    /// ネットワークエラー
    NetworkError,
    /// タイムアウト
    Timeout,
}
```

### DownloadState（ダウンロード状態）

```rust
/// ダウンロード状態管理
#[derive(Debug, Clone)]
pub struct DownloadState {
    /// モデルID
    pub model_id: String,
    /// ダウンロード開始時刻
    pub started_at: DateTime<Utc>,
    /// 現在の進捗率（0.0 - 1.0）
    pub progress: f32,
    /// ダウンロード済みバイト数
    pub downloaded_bytes: u64,
    /// 総バイト数
    pub total_bytes: u64,
    /// ステータス
    pub status: DownloadStatus,
}

/// ダウンロードステータス
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    /// 待機中（ロック待ち）
    Waiting,
    /// ダウンロード中
    InProgress,
    /// 完了
    Completed,
    /// 失敗
    Failed,
}
```

## 検証ルール表

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `id` | 空文字不可、英数字とハイフンのみ | "Invalid model ID format" |
| `source` | 定義済みソースのみ | "Unknown model source" |
| `repo_id` | HFソースの場合必須 | "Repository ID required for HuggingFace models" |
| `artifacts` | 最低1つ必要 | "At least one artifact required" |
| `required_vram_mb` | 0より大きい | "Required VRAM must be positive" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                    supported_models.json                     │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ SupportedModel                                       │    │
│  │  - id: String                                        │    │
│  │  - source: ModelSource                               │    │
│  │  - repo_id: Option<String>                           │    │
│  │  - artifacts: Vec<Artifact>                          │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      ModelResolver                           │
│  resolve(model_id) -> ModelResolutionResult                  │
│                                                              │
│  ┌─────────────────┐   ┌─────────────────┐                  │
│  │ ローカルキャッシュ │   │ ダウンローダー   │                  │
│  │ ~/.llm-router/  │   │ HF Hub API      │                  │
│  │ models/         │   │ Router Proxy    │                  │
│  └─────────────────┘   └─────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      DownloadState                           │
│  - 進捗追跡                                                   │
│  - 重複防止                                                   │
│  - ロックファイル管理                                          │
└─────────────────────────────────────────────────────────────┘
```

## ファイルシステム構造

```text
~/.llm-router/
├── models/
│   ├── llama-3.2-1b/
│   │   └── model.gguf
│   ├── mistral-7b/
│   │   └── model.gguf
│   └── .locks/
│       └── llama-3.2-1b.lock
└── supported_models.json
```
