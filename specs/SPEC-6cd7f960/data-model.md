# データモデル: 対応モデルリスト型管理

## エンティティ定義

### SupportedModel

```rust
/// 対応モデル定義（supported_models.jsonから読み込み）
#[derive(Serialize, Deserialize)]
pub struct SupportedModel {
    /// モデル識別子（一意）
    pub id: String,
    /// 表示名
    pub name: String,
    /// 説明文
    pub description: String,
    /// HuggingFaceリポジトリID
    pub repo: String,
    /// 推奨ファイル名（GGUF等）
    pub recommended_filename: String,
    /// ファイルサイズ（バイト）
    pub size_bytes: u64,
    /// 必要VRAMサイズ（バイト）
    pub required_memory_bytes: u64,
    /// タグ一覧
    pub tags: Vec<String>,
    /// 対応機能
    pub capabilities: Vec<ModelCapability>,
    /// 量子化形式
    pub quantization: String,
    /// パラメータ数（例: "7B"）
    pub parameter_count: String,
    /// ファイル形式
    pub format: ModelFormat,
    /// 対応エンジン
    pub engine: String,
    /// 対応プラットフォーム
    pub platforms: Vec<Platform>,
}

#[derive(Serialize, Deserialize)]
pub enum ModelCapability {
    TextGeneration,
    ImageUnderstanding,
    ImageGeneration,
    SpeechToText,
    TextToSpeech,
}

#[derive(Serialize, Deserialize)]
pub enum ModelFormat {
    Gguf,
    Safetensors,
    PyTorch,
}

#[derive(Serialize, Deserialize)]
pub enum Platform {
    Linux,
    Windows,
    MacOS,
}
```

### ModelListResponse

```rust
/// GET /v0/models レスポンス
pub struct ModelListResponse {
    pub models: Vec<ModelWithStatus>,
}

pub struct ModelWithStatus {
    /// 基本情報
    pub model: SupportedModel,
    /// 現在の状態
    pub status: ModelStatus,
    /// HF動的情報（オプション）
    pub hf_stats: Option<HfStats>,
    /// ダウンロード進捗（downloading時のみ）
    pub download_progress: Option<f32>,
    /// 利用可能なノード数
    pub available_nodes: u32,
}

#[derive(Serialize, Deserialize)]
pub enum ModelStatus {
    /// Hub上で利用可能（未ダウンロード）
    Available,
    /// 登録済み（ダウンロード待ち）
    Registered,
    /// ダウンロード中
    Downloading,
    /// 利用可能（ダウンロード済み）
    Ready,
    /// エラー状態
    Error { message: String },
}

pub struct HfStats {
    /// ダウンロード数
    pub downloads: u64,
    /// スター数
    pub stars: u64,
    /// 最終更新日時
    pub last_modified: DateTime<Utc>,
}
```

### ModelRegisterRequest

```rust
/// POST /v0/models/register リクエスト
pub struct ModelRegisterRequest {
    /// モデルID（supported_models.jsonから）
    pub model_id: String,
}
```

### NodeModelState

```rust
/// ノード上のモデル状態
pub struct NodeModelState {
    /// ノードID
    pub node_id: String,
    /// モデルID
    pub model_id: String,
    /// ライフサイクル状態
    pub lifecycle_status: LifecycleStatus,
    /// ファイルパス（ダウンロード済みの場合）
    pub file_path: Option<PathBuf>,
    /// ダウンロード進捗（0.0〜1.0）
    pub progress: Option<f32>,
    /// 最終更新時刻
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub enum LifecycleStatus {
    /// 同期待ち
    Pending,
    /// ダウンロード中
    Downloading,
    /// ロード中
    Loading,
    /// 利用可能
    Ready,
    /// エラー
    Error { message: String },
}
```

### WebSocketEvent

```rust
/// WebSocket経由で配信されるイベント
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum WsEvent {
    /// モデル状態変更
    ModelStatusChanged {
        model_id: String,
        status: ModelStatus,
        progress: Option<f32>,
    },
    /// ノード状態変更
    NodeStatusChanged {
        node_id: String,
        status: NodeStatus,
        models: Vec<NodeModelState>,
    },
    /// メトリクス更新
    MetricsUpdated {
        node_id: String,
        gpu_usage: f32,
        memory_usage: f32,
        request_count: u64,
    },
}
```

## 検証ルール

| エンティティ | フィールド | ルール |
|-------------|-----------|--------|
| SupportedModel | id | 空でないこと、英数字とハイフンのみ |
| SupportedModel | repo | HuggingFaceリポジトリ形式（`org/repo`） |
| SupportedModel | size_bytes | 0より大きいこと |
| SupportedModel | required_memory_bytes | size_bytes以上であること |
| ModelRegisterRequest | model_id | supported_models.jsonに存在すること |
| NodeModelState | progress | 0.0〜1.0の範囲 |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                       supported_models.json                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   SupportedModel[]                       │    │
│  │  - llama-3.2-1b                                          │    │
│  │  - qwen2.5-coder-7b                                      │    │
│  │  - llava-1.6-7b                                          │    │
│  │  - ...                                                   │    │
│  └─────────────────────────────────────────────────────────┘    │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               │ load at startup
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Router                                  │
│  ┌────────────────┐    ┌────────────────┐    ┌──────────────┐   │
│  │ SupportedModel │    │ RegisteredModel│    │   HfCache    │   │
│  │    (static)    │    │   (database)   │    │ (in-memory)  │   │
│  └───────┬────────┘    └───────┬────────┘    └──────┬───────┘   │
│          │                     │                     │          │
│          └─────────────────────┼─────────────────────┘          │
│                                │                                │
│                                ▼                                │
│                    ┌───────────────────────┐                    │
│                    │ GET /v0/models        │                    │
│                    │ ModelWithStatus[]     │                    │
│                    └───────────┬───────────┘                    │
└────────────────────────────────│────────────────────────────────┘
                                 │
                                 │ WebSocket
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Dashboard (Frontend)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                     Model Hub Tab                        │    │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐    │    │
│  │  │ LLaMA 3.2│ │Qwen2.5   │ │ LLaVA    │ │  ...     │    │    │
│  │  │ [Pull]   │ │[Ready ✓] │ │[Download]│ │          │    │    │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘    │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## supported_models.json 例

```json
[
  {
    "id": "llama-3.2-1b",
    "name": "LLaMA 3.2 1B",
    "description": "Lightweight model for quick inference",
    "repo": "bartowski/Llama-3.2-1B-Instruct-GGUF",
    "recommended_filename": "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    "size_bytes": 800000000,
    "required_memory_bytes": 1600000000,
    "tags": ["chat", "instruct", "lightweight"],
    "capabilities": ["TextGeneration"],
    "quantization": "Q4_K_M",
    "parameter_count": "1B",
    "format": "Gguf",
    "engine": "llama.cpp",
    "platforms": ["Linux", "Windows", "MacOS"]
  },
  {
    "id": "llava-1.6-7b",
    "name": "LLaVA 1.6 7B",
    "description": "Vision-language model for image understanding",
    "repo": "cjpais/llava-1.6-mistral-7b-gguf",
    "recommended_filename": "llava-v1.6-mistral-7b.Q4_K_M.gguf",
    "size_bytes": 4500000000,
    "required_memory_bytes": 8000000000,
    "tags": ["vision", "multimodal", "chat"],
    "capabilities": ["TextGeneration", "ImageUnderstanding"],
    "quantization": "Q4_K_M",
    "parameter_count": "7B",
    "format": "Gguf",
    "engine": "llama.cpp",
    "platforms": ["Linux", "Windows", "MacOS"]
  }
]
```
