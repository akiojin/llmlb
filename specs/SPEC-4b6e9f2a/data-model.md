# データモデル: クラウドモデルプレフィックスルーティング

## エンティティ定義

### クラウドプロバイダー

```rust
// router/src/cloud/provider.rs

use serde::{Deserialize, Serialize};

/// 対応クラウドプロバイダー
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    OpenAI,
    Google,
    Anthropic,
}

impl CloudProvider {
    /// モデル名のプレフィックス
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai:",
            Self::Google => "google:",
            Self::Anthropic => "anthropic:",
        }
    }

    /// デフォルトのAPIエンドポイント
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta",
            Self::Anthropic => "https://api.anthropic.com/v1",
        }
    }

    /// 環境変数プレフィックス
    pub fn env_prefix(&self) -> &'static str {
        match self {
            Self::OpenAI => "OPENAI",
            Self::Google => "GOOGLE",
            Self::Anthropic => "ANTHROPIC",
        }
    }
}
```

### プロバイダー設定

```rust
// router/src/cloud/config.rs

/// プロバイダー接続設定
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// APIキー
    pub api_key: String,

    /// APIベースURL
    pub base_url: String,

    /// タイムアウト（秒）
    pub timeout_secs: u64,
}

impl ProviderConfig {
    /// 環境変数から設定を読み込む
    pub fn from_env(provider: CloudProvider) -> Option<Self> {
        let prefix = provider.env_prefix();

        let api_key = std::env::var(format!("{}_API_KEY", prefix)).ok()?;

        let base_url = std::env::var(format!("{}_BASE_URL", prefix))
            .unwrap_or_else(|_| provider.default_base_url().to_string());

        let timeout_secs = std::env::var(format!("{}_TIMEOUT_SECS", prefix))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Some(Self {
            api_key,
            base_url,
            timeout_secs,
        })
    }
}

/// 全プロバイダーの設定
#[derive(Debug, Clone, Default)]
pub struct CloudConfig {
    pub openai: Option<ProviderConfig>,
    pub google: Option<ProviderConfig>,
    pub anthropic: Option<ProviderConfig>,
}

impl CloudConfig {
    /// 環境変数から設定を読み込む
    pub fn from_env() -> Self {
        Self {
            openai: ProviderConfig::from_env(CloudProvider::OpenAI),
            google: ProviderConfig::from_env(CloudProvider::Google),
            anthropic: ProviderConfig::from_env(CloudProvider::Anthropic),
        }
    }

    /// プロバイダーの設定を取得
    pub fn get(&self, provider: CloudProvider) -> Option<&ProviderConfig> {
        match provider {
            CloudProvider::OpenAI => self.openai.as_ref(),
            CloudProvider::Google => self.google.as_ref(),
            CloudProvider::Anthropic => self.anthropic.as_ref(),
        }
    }

    /// 設定済みプロバイダーの一覧
    pub fn configured_providers(&self) -> Vec<CloudProvider> {
        let mut providers = Vec::new();
        if self.openai.is_some() {
            providers.push(CloudProvider::OpenAI);
        }
        if self.google.is_some() {
            providers.push(CloudProvider::Google);
        }
        if self.anthropic.is_some() {
            providers.push(CloudProvider::Anthropic);
        }
        providers
    }
}
```

### ルーティング結果

```rust
// router/src/cloud/routing.rs

/// ルーティング先
#[derive(Debug, Clone)]
pub enum RouteTarget {
    /// ローカルノード
    Local { model: String },

    /// クラウドプロバイダー
    Cloud {
        provider: CloudProvider,
        model: String,
    },
}

impl RouteTarget {
    /// モデル名からルーティング先を決定
    pub fn from_model_name(model: &str) -> Self {
        for provider in [
            CloudProvider::OpenAI,
            CloudProvider::Google,
            CloudProvider::Anthropic,
        ] {
            if let Some(name) = model.strip_prefix(provider.prefix()) {
                return Self::Cloud {
                    provider,
                    model: name.to_string(),
                };
            }
        }

        Self::Local {
            model: model.to_string(),
        }
    }

    /// クラウドルーティングかどうか
    pub fn is_cloud(&self) -> bool {
        matches!(self, Self::Cloud { .. })
    }
}
```

### クラウドリクエスト

```rust
// router/src/cloud/request.rs

use serde::{Deserialize, Serialize};

/// クラウドAPIへのチャットリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudChatRequest {
    /// モデル名（プレフィックスなし）
    pub model: String,

    /// メッセージ配列
    pub messages: Vec<ChatMessage>,

    /// ストリーミング有効化
    #[serde(default)]
    pub stream: bool,

    /// 最大トークン数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// 温度パラメータ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// チャットメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}
```

### クラウドエラー

```rust
// router/src/cloud/error.rs

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

/// クラウドAPIエラー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudError {
    pub error: CloudErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudErrorDetail {
    /// エラーメッセージ
    pub message: String,

    /// エラータイプ
    #[serde(rename = "type")]
    pub error_type: String,

    /// エラーコード
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// プロバイダー名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// クラウドエラー種別
#[derive(Debug, Clone)]
pub enum CloudErrorKind {
    /// APIキー未設定
    ApiKeyMissing { provider: CloudProvider },

    /// APIキー無効
    ApiKeyInvalid { provider: CloudProvider },

    /// レート制限
    RateLimited { retry_after: Option<u64> },

    /// モデル不存在
    ModelNotFound { model: String },

    /// 不明なプレフィックス
    UnknownPrefix { prefix: String },

    /// プロバイダーエラー
    ProviderError { status: u16, message: String },

    /// タイムアウト
    Timeout,
}

impl CloudErrorKind {
    /// HTTPステータスコード
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::ApiKeyMissing { .. } => StatusCode::UNAUTHORIZED,
            Self::ApiKeyInvalid { .. } => StatusCode::UNAUTHORIZED,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::ModelNotFound { .. } => StatusCode::NOT_FOUND,
            Self::UnknownPrefix { .. } => StatusCode::BAD_REQUEST,
            Self::ProviderError { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }

    /// エラータイプ文字列
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::ApiKeyMissing { .. } => "authentication_error",
            Self::ApiKeyInvalid { .. } => "authentication_error",
            Self::RateLimited { .. } => "rate_limit_error",
            Self::ModelNotFound { .. } => "invalid_request_error",
            Self::UnknownPrefix { .. } => "invalid_request_error",
            Self::ProviderError { .. } => "provider_error",
            Self::Timeout => "timeout_error",
        }
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|--------------------|
| `model` | 空でない | "Model name is required" |
| `model` | 有効なプレフィックス | "Unknown cloud provider prefix: {prefix}" |
| `api_key` | 設定済み | "{provider} API key not configured" |
| `messages` | 1件以上 | "At least one message is required" |
| `timeout_secs` | 1以上300以下 | "Timeout must be between 1 and 300 seconds" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────────┐
│                    クラウドルーティングシステム                       │
│                                                                     │
│  ┌──────────────┐                                                   │
│  │   Request    │ model: "openai:gpt-4.1"                           │
│  └──────┬───────┘                                                   │
│         │                                                           │
│         ▼                                                           │
│  ┌──────────────┐     ┌─────────────────┐                          │
│  │ RouteTarget  │────→│  CloudProvider  │                          │
│  │ ::from_model │     │  - OpenAI       │                          │
│  └──────┬───────┘     │  - Google       │                          │
│         │             │  - Anthropic    │                          │
│         │             └─────────────────┘                          │
│         ▼                                                           │
│  ┌──────────────────────────────────────┐                          │
│  │          RouteTarget                  │                          │
│  │  ┌────────────┐  ┌────────────────┐  │                          │
│  │  │   Local    │  │     Cloud      │  │                          │
│  │  │  { model } │  │ { provider,    │  │                          │
│  │  │            │  │   model }      │  │                          │
│  │  └─────┬──────┘  └───────┬────────┘  │                          │
│  └────────┼─────────────────┼───────────┘                          │
│           │                 │                                       │
│           ▼                 ▼                                       │
│  ┌────────────────┐  ┌──────────────────┐                          │
│  │  Local Node    │  │  ProviderConfig  │                          │
│  │  (allm)    │  │  - api_key       │                          │
│  │                │  │  - base_url      │                          │
│  │                │  │  - timeout_secs  │                          │
│  └────────────────┘  └────────┬─────────┘                          │
│                               │                                     │
│                               ▼                                     │
│                      ┌────────────────────┐                        │
│                      │   Cloud API        │                        │
│                      │  - OpenAI API      │                        │
│                      │  - Google AI API   │                        │
│                      │  - Anthropic API   │                        │
│                      └────────────────────┘                        │
└─────────────────────────────────────────────────────────────────────┘
```

## ステータスAPI形式

### GET /v0/status（クラウド情報含む）

```json
{
  "status": "ok",
  "version": "0.1.0",
  "cloud_providers": {
    "openai": {
      "configured": true,
      "base_url": "https://api.openai.com/v1"
    },
    "google": {
      "configured": false
    },
    "anthropic": {
      "configured": true,
      "base_url": "https://api.anthropic.com/v1"
    }
  }
}
```

## メトリクス形式

```text
# クラウドAPIリクエスト数
llm_router_cloud_requests_total{provider="openai",model="gpt-4.1",status="200"} 150
llm_router_cloud_requests_total{provider="anthropic",model="claude-3-opus",status="200"} 80
llm_router_cloud_requests_total{provider="openai",model="gpt-4.1",status="429"} 5

# クラウドAPIレイテンシ
llm_router_cloud_latency_seconds_bucket{provider="openai",le="0.5"} 100
llm_router_cloud_latency_seconds_bucket{provider="openai",le="1.0"} 140
llm_router_cloud_latency_seconds_bucket{provider="openai",le="2.5"} 150
llm_router_cloud_latency_seconds_sum{provider="openai"} 95.5
llm_router_cloud_latency_seconds_count{provider="openai"} 150
```
