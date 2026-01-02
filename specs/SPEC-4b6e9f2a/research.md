# 技術リサーチ: クラウドモデルプレフィックスルーティング

## リサーチ課題

1. クラウドAPI統合の設計パターン
2. ストリーミングプロキシの実装方式
3. 認証情報の安全な管理
4. エラーハンドリングと可観測性

## 1. クラウドAPI統合パターン

### 決定

**プロキシパターン**（ルーターが仲介）を採用

### 理由

- ルーターがすべてのリクエストを一元管理でき、ログ・メトリクス収集が容易
- クライアントは単一エンドポイントのみ知っていればよい
- APIキーをクライアント側に露出させない

### 代替案比較

| パターン | 説明 | メリット | デメリット |
|---------|------|---------|-----------|
| プロキシ | ルーター経由で転送 | 一元管理、セキュリティ | レイテンシ追加 |
| リダイレクト | 302でクラウドURLを返す | レイテンシ最小 | APIキー露出リスク |
| SDK統合 | 各SDKを内包 | 型安全、エラー詳細 | 依存肥大化 |

### 実装方法

```rust
// router/src/cloud/mod.rs

pub enum CloudProvider {
    OpenAI,
    Google,
    Anthropic,
}

impl CloudProvider {
    /// モデル名からプレフィックスを解析
    pub fn from_model_name(model: &str) -> Option<(Self, String)> {
        if let Some(name) = model.strip_prefix("openai:") {
            Some((Self::OpenAI, name.to_string()))
        } else if let Some(name) = model.strip_prefix("google:") {
            Some((Self::Google, name.to_string()))
        } else if let Some(name) = model.strip_prefix("anthropic:") {
            Some((Self::Anthropic, name.to_string()))
        } else {
            None
        }
    }

    /// ベースURLを取得
    pub fn base_url(&self) -> &'static str {
        match self {
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Google => "https://generativelanguage.googleapis.com/v1beta",
            Self::Anthropic => "https://api.anthropic.com/v1",
        }
    }
}
```

## 2. ストリーミングプロキシ実装

### 決定

**SSEパススルー方式**を採用（変換なし）

### 理由

- OpenAIとAnthropicはSSE形式で互換性が高い
- 変換処理によるレイテンシ・複雑さを回避
- クラウドAPIのレスポンスをそのまま転送

### 代替案比較

| 方式 | 説明 | メリット | デメリット |
|------|------|---------|-----------|
| パススルー | そのまま転送 | シンプル、低レイテンシ | フォーマット差異対応困難 |
| 変換 | 統一形式に変換 | 一貫性 | 複雑、レイテンシ |
| バッファリング | 全て受信後に返す | 変換容易 | ストリーミング無効化 |

### 実装方法

```rust
// router/src/cloud/streaming.rs

use axum::response::sse::{Event, Sse};
use futures::Stream;
use reqwest::Response;

pub async fn proxy_stream(
    response: Response,
) -> Sse<impl Stream<Item = Result<Event, anyhow::Error>>> {
    let stream = async_stream::stream! {
        let mut lines = response.bytes_stream();
        while let Some(chunk) = lines.next().await {
            match chunk {
                Ok(bytes) => {
                    // SSEイベントをそのまま転送
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            yield Ok(Event::default().data(&line[6..]));
                        }
                    }
                }
                Err(e) => {
                    yield Err(anyhow::anyhow!("Stream error: {}", e));
                    break;
                }
            }
        }
    };

    Sse::new(stream)
}
```

## 3. 認証情報管理

### 決定

**環境変数による設定**を採用

### 理由

- Kubernetesシークレット、AWS Secrets Managerとの統合が容易
- 設定ファイルへのAPIキー記載を回避
- 12-Factor Appに準拠

### 環境変数設計

| 変数名 | 必須 | デフォルト | 説明 |
|--------|------|----------|------|
| `OPENAI_API_KEY` | ○ | - | OpenAI APIキー |
| `OPENAI_BASE_URL` | - | `https://api.openai.com/v1` | カスタムエンドポイント |
| `GOOGLE_API_KEY` | ○ | - | Google AI APIキー |
| `GOOGLE_API_BASE_URL` | - | `https://...googleapis.com/v1beta` | カスタムエンドポイント |
| `ANTHROPIC_API_KEY` | ○ | - | Anthropic APIキー |
| `ANTHROPIC_API_BASE_URL` | - | `https://api.anthropic.com/v1` | カスタムエンドポイント |

### 実装方法

```rust
// router/src/cloud/config.rs

#[derive(Debug, Clone)]
pub struct CloudConfig {
    pub openai: Option<ProviderConfig>,
    pub google: Option<ProviderConfig>,
    pub anthropic: Option<ProviderConfig>,
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: String,
}

impl CloudConfig {
    pub fn from_env() -> Self {
        Self {
            openai: Self::load_provider("OPENAI", "https://api.openai.com/v1"),
            google: Self::load_provider("GOOGLE", "https://generativelanguage.googleapis.com/v1beta"),
            anthropic: Self::load_provider("ANTHROPIC", "https://api.anthropic.com/v1"),
        }
    }

    fn load_provider(prefix: &str, default_url: &str) -> Option<ProviderConfig> {
        let api_key = std::env::var(format!("{}_API_KEY", prefix)).ok()?;
        let base_url = std::env::var(format!("{}_BASE_URL", prefix))
            .unwrap_or_else(|_| default_url.to_string());
        Some(ProviderConfig { api_key, base_url })
    }
}
```

## 4. エラーハンドリング

### 決定

**クラウドAPIエラーを透過的に返却**する

### 理由

- クライアントがエラー詳細を把握しやすい
- リトライ判断をクライアントに委ねる
- ルーター側での複雑なリトライロジックを回避

### エラーマッピング

| クラウドステータス | ルーター応答 | 対応 |
|-------------------|-------------|------|
| 401 Unauthorized | 401 | APIキー無効 |
| 403 Forbidden | 403 | 権限不足 |
| 404 Not Found | 404 | モデル不存在 |
| 429 Rate Limited | 429 | レート制限（リトライなし） |
| 500+ Server Error | 502 | クラウド側エラー |
| Timeout | 504 | タイムアウト |

### 実装方法

```rust
// router/src/cloud/error.rs

use axum::http::StatusCode;

pub enum CloudError {
    ApiKeyMissing { provider: String },
    ApiKeyInvalid { provider: String },
    RateLimited { retry_after: Option<u64> },
    ModelNotFound { model: String },
    ProviderError { status: u16, message: String },
    Timeout,
}

impl CloudError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::ApiKeyMissing { .. } => StatusCode::UNAUTHORIZED,
            Self::ApiKeyInvalid { .. } => StatusCode::UNAUTHORIZED,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::ModelNotFound { .. } => StatusCode::NOT_FOUND,
            Self::ProviderError { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }
}
```

## 5. 可観測性

### メトリクス設計

```rust
// router/src/cloud/metrics.rs

use prometheus::{Counter, Histogram, register_counter_vec, register_histogram_vec};

lazy_static! {
    pub static ref CLOUD_REQUESTS: CounterVec = register_counter_vec!(
        "llm_router_cloud_requests_total",
        "Total cloud API requests",
        &["provider", "model", "status"]
    ).unwrap();

    pub static ref CLOUD_LATENCY: HistogramVec = register_histogram_vec!(
        "llm_router_cloud_latency_seconds",
        "Cloud API request latency",
        &["provider"],
        vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).unwrap();
}
```

### ログ出力

```json
{
  "ts": "2025-12-01T12:00:00.000Z",
  "level": "info",
  "category": "api",
  "msg": "Cloud request completed",
  "provider": "openai",
  "model": "gpt-4.1",
  "status": 200,
  "latency_ms": 1234,
  "request_id": "abc123"
}
```

## 参考リソース

- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [Google AI API](https://ai.google.dev/api)
- [Anthropic API Reference](https://docs.anthropic.com/en/api)
- [axum SSE](https://docs.rs/axum/latest/axum/response/sse/)
- [reqwest Streaming](https://docs.rs/reqwest/latest/reqwest/struct.Response.html#method.bytes_stream)
