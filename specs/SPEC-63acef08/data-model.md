# データモデル: 統一APIプロキシ

## エンティティ定義

### プロキシリクエスト

```rust
// llmlb/src/api/proxy.rs

use serde::{Deserialize, Serialize};

/// チャット完了リクエスト（OpenAI互換）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    /// モデル名
    pub model: String,

    /// メッセージ履歴
    pub messages: Vec<ChatMessage>,

    /// ストリーミングフラグ
    #[serde(default)]
    pub stream: bool,

    /// 生成温度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// 最大トークン数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Top-pサンプリング
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// その他のパラメータ
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

/// チャットメッセージ
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    /// ロール（system, user, assistant）
    pub role: String,

    /// メッセージ内容
    pub content: String,
}
```

### プロキシレスポンス

```rust
// llmlb/src/api/proxy.rs

/// チャット完了レスポンス（OpenAI互換）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionResponse {
    /// レスポンスID
    pub id: String,

    /// オブジェクトタイプ
    pub object: String,

    /// 作成時刻（Unix timestamp）
    pub created: u64,

    /// モデル名
    pub model: String,

    /// 選択肢
    pub choices: Vec<ChatChoice>,

    /// 使用トークン情報
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// 選択肢
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatChoice {
    /// インデックス
    pub index: u32,

    /// メッセージ
    pub message: ChatMessage,

    /// 終了理由
    pub finish_reason: Option<String>,
}

/// トークン使用量
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    /// プロンプトトークン数
    pub prompt_tokens: u32,

    /// 完了トークン数
    pub completion_tokens: u32,

    /// 総トークン数
    pub total_tokens: u32,
}
```

### ストリーミングチャンク

```rust
// llmlb/src/api/streaming.rs

/// ストリーミングチャンク（SSE）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionChunk {
    /// チャンクID
    pub id: String,

    /// オブジェクトタイプ
    pub object: String,

    /// 作成時刻
    pub created: u64,

    /// モデル名
    pub model: String,

    /// 選択肢（デルタ）
    pub choices: Vec<ChunkChoice>,
}

/// チャンクの選択肢
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChunkChoice {
    /// インデックス
    pub index: u32,

    /// デルタ（差分）
    pub delta: ChunkDelta,

    /// 終了理由
    pub finish_reason: Option<String>,
}

/// デルタ内容
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChunkDelta {
    /// ロール（最初のチャンクのみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// コンテンツ（差分テキスト）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}
```

### Embeddings

```rust
// llmlb/src/api/embeddings.rs

/// Embeddingsリクエスト
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingsRequest {
    /// 入力テキスト（単一または複数）
    pub input: EmbeddingInput,

    /// モデル名
    pub model: String,

    /// エンコーディング形式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
}

/// 入力テキスト
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    /// 単一テキスト
    Single(String),

    /// 複数テキスト
    Multiple(Vec<String>),
}

/// Embeddingsレスポンス
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingsResponse {
    /// オブジェクトタイプ
    pub object: String,

    /// 埋め込みデータ
    pub data: Vec<EmbeddingData>,

    /// モデル名
    pub model: String,

    /// 使用トークン情報
    pub usage: EmbeddingUsage,
}

/// 埋め込みデータ
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingData {
    /// オブジェクトタイプ
    pub object: String,

    /// 埋め込みベクトル
    pub embedding: Vec<f32>,

    /// インデックス
    pub index: u32,
}

/// 埋め込み使用量
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingUsage {
    /// プロンプトトークン数
    pub prompt_tokens: u32,

    /// 総トークン数
    pub total_tokens: u32,
}
```

### プロキシ設定

```rust
// llmlb/src/proxy/config.rs

use std::time::Duration;

/// プロキシ設定
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// リクエストタイムアウト
    pub request_timeout: Duration,

    /// 接続タイムアウト
    pub connect_timeout: Duration,

    /// 同時リクエスト上限
    pub max_concurrent_requests: usize,

    /// リトライ回数
    pub max_retries: u32,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(5),
            max_concurrent_requests: 100,
            max_retries: 0,  // フェイルオーバーなし（現在）
        }
    }
}
```

### プロキシエラー

```rust
// llmlb/src/api/error.rs

/// プロキシエラー
#[derive(Debug, Clone)]
pub enum ProxyError {
    /// 利用可能なノードがない
    NoAvailableNodes,

    /// ノードへの接続失敗
    NodeConnectionFailed {
        runtime_id: String,
        reason: String,
    },

    /// タイムアウト
    Timeout {
        timeout_secs: u64,
    },

    /// ノードからのエラーレスポンス
    NodeError {
        runtime_id: String,
        status: u16,
        body: String,
    },

    /// 不正なリクエスト
    InvalidRequest {
        message: String,
    },
}

impl ProxyError {
    /// HTTPステータスコードを取得
    pub fn status_code(&self) -> u16 {
        match self {
            Self::NoAvailableNodes => 503,
            Self::NodeConnectionFailed { .. } => 502,
            Self::Timeout { .. } => 504,
            Self::NodeError { status, .. } => *status,
            Self::InvalidRequest { .. } => 400,
        }
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `model` | 非空文字列 | "Model name is required" |
| `messages` | 1件以上 | "At least one message is required" |
| `messages[].role` | system/user/assistant | "Invalid role" |
| `temperature` | 0.0 - 2.0 | "Temperature must be between 0 and 2" |
| `max_tokens` | 1 - 128000 | "Max tokens must be between 1 and 128000" |
| `top_p` | 0.0 - 1.0 | "Top-p must be between 0 and 1" |
| `input` (embeddings) | 非空 | "Input text is required" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                        Proxy Service                             │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    リクエストフロー                           ││
│  │                                                              ││
│  │  [Client Request]                                            ││
│  │         │                                                    ││
│  │         ▼                                                    ││
│  │  ┌────────────────┐                                         ││
│  │  │ Request Parse  │ ChatCompletionRequest                   ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ Node Selection │ LoadBalancer.select()                   ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ HTTP Forward   │ reqwest → Node                          ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │    ┌─────┴─────┐                                            ││
│  │    ▼           ▼                                            ││
│  │  [Normal]  [Stream]                                         ││
│  │    │           │                                            ││
│  │    ▼           ▼                                            ││
│  │  ChatCompletionResponse  ChatCompletionChunk...             ││
│  │    │           │                                            ││
│  │    └─────┬─────┘                                            ││
│  │          ▼                                                   ││
│  │  [Client Response]                                           ││
│  │                                                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ ProxyConfig  │  │ ProxyError   │  │ StreamState  │          │
│  │ - timeout    │  │ - NoNodes    │  │ - chunks     │          │
│  │ - max_conc   │  │ - Timeout    │  │ - done       │          │
│  │ - retries    │  │ - NodeError  │  │ - error      │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## 環境変数

```bash
# プロキシタイムアウト
LLMLB_REQUEST_TIMEOUT=60     # リクエストタイムアウト（秒）
LLMLB_CONNECT_TIMEOUT=5      # 接続タイムアウト（秒）

# 同時リクエスト制限
LLMLB_MAX_CONCURRENT=100     # 最大同時リクエスト数
```

## メトリクス形式

```text
# プロキシリクエスト
llmlb_proxy_requests_total{endpoint="/v1/chat/completions",status="200"} 5000
llmlb_proxy_requests_total{endpoint="/v1/embeddings",status="200"} 1000
llmlb_proxy_requests_total{endpoint="/v1/chat/completions",status="503"} 50

# レイテンシ
llmlb_proxy_duration_seconds_bucket{endpoint="/v1/chat/completions",le="1"} 2000
llmlb_proxy_duration_seconds_bucket{endpoint="/v1/chat/completions",le="5"} 4500
llmlb_proxy_duration_seconds_bucket{endpoint="/v1/chat/completions",le="30"} 5000

# ストリーミング
llmlb_streaming_connections_active 15
llmlb_streaming_chunks_total{runtime_id="node-1"} 50000
```
