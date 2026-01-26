# データモデル: クラウドプロバイダーモデル一覧統合

**機能ID**: `SPEC-82491000` | **日付**: 2025-12-25

## エンティティ定義

### CloudModelInfo

クラウドプロバイダーから取得したモデル情報を表す。

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| id | String | ✅ | プレフィックス付きモデルID（例: `openai:gpt-4o`） |
| object | String | ✅ | 固定値 `"model"` |
| created | i64 | ✅ | 作成日時（Unixタイムスタンプ、不明時は0） |
| owned_by | String | ✅ | プロバイダー名（`openai`, `google`, `anthropic`） |

**Rust定義**:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}
```

### CloudModelsCache

キャッシュされたクラウドモデル一覧を管理する。

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| models | Vec&lt;CloudModelInfo&gt; | ✅ | キャッシュされたモデル一覧 |
| fetched_at | DateTime&lt;Utc&gt; | ✅ | 取得時刻 |

**Rust定義**:

```rust
pub struct CloudModelsCache {
    pub models: Vec<CloudModelInfo>,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}
```

**キャッシュ定数**:

```rust
/// キャッシュTTL: 24時間
pub const CLOUD_MODELS_CACHE_TTL_SECS: u64 = 86400;

/// 各プロバイダーAPIタイムアウト: 10秒
pub const CLOUD_MODELS_FETCH_TIMEOUT_SECS: u64 = 10;
```

## プロバイダー固有レスポンス

### OpenAI API レスポンス

```rust
#[derive(Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Deserialize)]
struct OpenAIModel {
    id: String,
    object: String,
    created: i64,
    owned_by: String,
}
```

### Google AI API レスポンス

```rust
#[derive(Deserialize)]
struct GoogleModelsResponse {
    models: Vec<GoogleModel>,
}

#[derive(Deserialize)]
struct GoogleModel {
    name: String,           // "models/gemini-2.0-flash"
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}
```

### Anthropic API レスポンス

```rust
#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(rename = "type")]
    model_type: String,
    display_name: Option<String>,
    created_at: Option<String>,  // ISO 8601形式
}
```

## 変換ルール

### プレフィックス付与

| プロバイダー | 入力例 | 出力例 |
|-------------|--------|--------|
| OpenAI | `gpt-4o` | `openai:gpt-4o` |
| Google | `models/gemini-2.0-flash` | `google:gemini-2.0-flash` |
| Anthropic | `claude-sonnet-4-20250514` | `anthropic:claude-sonnet-4-20250514` |

### Google モデル名正規化

`models/` プレフィックスを除去:

```rust
fn normalize_google_model_name(name: &str) -> String {
    name.trim_start_matches("models/").to_string()
}
```

### Anthropic 日時変換

ISO 8601形式をUnixタイムスタンプに変換:

```rust
fn parse_anthropic_created_at(created_at: Option<&str>) -> i64 {
    created_at
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}
```

## 状態遷移

### キャッシュ状態

```text
[Empty] ---(初回リクエスト)---> [Fetching] ---(成功)---> [Valid]
                                    |
                                    +---(失敗)---> [Empty]

[Valid] ---(TTL経過)---> [Stale] ---(リフレッシュ成功)---> [Valid]
                            |
                            +---(リフレッシュ失敗)---> [Stale] (フォールバック)
```

## 関連エンティティ（既存）

### 既存のモデル情報構造（参考）

`llmlb/src/api/openai.rs` の `list_models()` で使用される既存構造:

```rust
// 既存のローカルモデル情報（変更なし）
{
    "id": "llama-3.2",
    "object": "model",
    "created": 0,
    "owned_by": "lb",
    "capabilities": { ... },
    "lifecycle_status": "registered",
    "ready": true
}
```

**統合後のレスポンス**:

```json
{
    "object": "list",
    "data": [
        // ローカルモデル（既存）
        {"id": "llama-3.2", "owned_by": "lb", ...},
        // クラウドモデル（新規追加）
        {"id": "openai:gpt-4o", "owned_by": "openai", ...},
        {"id": "google:gemini-2.0-flash", "owned_by": "google", ...},
        {"id": "anthropic:claude-sonnet-4-20250514", "owned_by": "anthropic", ...}
    ]
}
```

---

*Phase 1 データモデル完了*
