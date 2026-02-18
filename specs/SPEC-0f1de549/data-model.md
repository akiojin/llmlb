# データモデル: OpenAI互換API完全準拠

**機能ID**: `SPEC-0f1de549` | **日付**: 2026-01-05

## エンティティ定義

### TokenUsage

トークン使用量を表すエンティティ。

```cpp
struct TokenUsage {
    int prompt_tokens;      // 入力トークン数
    int completion_tokens;  // 生成トークン数
    int total_tokens;       // 合計トークン数
};
```

**検証ルール**:

- prompt_tokens >= 0
- completion_tokens >= 0
- total_tokens = prompt_tokens + completion_tokens

**JSON形式**:

```json
{
  "prompt_tokens": 10,
  "completion_tokens": 20,
  "total_tokens": 30
}
```

---

### LogprobInfo

単一トークンの確率情報を表すエンティティ。

```cpp
struct TopLogprob {
    std::string token;  // トークン文字列
    float logprob;      // log確率値
    std::vector<uint8_t> bytes;  // UTF-8バイト表現（オプション）
};

struct LogprobInfo {
    std::string token;                    // 生成されたトークン
    float logprob;                        // そのトークンのlog確率
    std::vector<uint8_t> bytes;           // UTF-8バイト表現
    std::vector<TopLogprob> top_logprobs; // 上位候補
};
```

**検証ルール**:

- logprob <= 0.0 (log確率は負の値)
- top_logprobs.size() <= 20

**JSON形式**:

```json
{
  "token": "Hello",
  "logprob": -0.5,
  "bytes": [72, 101, 108, 108, 111],
  "top_logprobs": [
    {"token": "Hello", "logprob": -0.5, "bytes": [72, 101, 108, 108, 111]},
    {"token": "Hi", "logprob": -1.2, "bytes": [72, 105]}
  ]
}
```

---

### InferenceParams 拡張

既存のInferenceParamsに追加するフィールド。

```cpp
struct InferenceParams {
    // 既存フィールド
    std::string model;
    float temperature = 1.0f;
    float top_p = 1.0f;
    int top_k = 40;
    int max_tokens = 256;
    std::vector<std::string> stop;
    float repeat_penalty = 1.0f;
    uint32_t seed = 0;

    // 新規追加フィールド
    float presence_penalty = 0.0f;   // -2.0 ~ 2.0
    float frequency_penalty = 0.0f;  // -2.0 ~ 2.0
    int n = 1;                       // 1 ~ 8
    bool logprobs = false;
    int top_logprobs = 0;            // 0 ~ 20
};
```

**検証ルール**:

- -2.0 <= presence_penalty <= 2.0
- -2.0 <= frequency_penalty <= 2.0
- 1 <= n <= 8
- 0 <= top_logprobs <= 20

---

### ResponseId

レスポンスIDの生成仕様。

**形式**: `{prefix}-{timestamp_hex}-{random_hex}`

| コンポーネント | 説明 | 例 |
|---------------|------|-----|
| prefix | エンドポイント固有 | chatcmpl, cmpl |
| timestamp_hex | ミリ秒タイムスタンプ（16進数） | 18d5a7b2c |
| random_hex | 4桁ランダム値 | a3f2 |

**例**: `chatcmpl-18d5a7b2c-a3f2`

---

## API レスポンス形式

### Chat Completions レスポンス

```json
{
  "id": "chatcmpl-18d5a7b2c-a3f2",
  "object": "chat.completion",
  "created": 1704067200,
  "model": "llama-3.1-8b",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello!"
      },
      "logprobs": {
        "content": [...]
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 5,
    "total_tokens": 15
  }
}
```

### Completions レスポンス

```json
{
  "id": "cmpl-18d5a7b2c-a3f2",
  "object": "text_completion",
  "created": 1704067200,
  "model": "llama-3.1-8b",
  "choices": [
    {
      "text": "Hello!",
      "index": 0,
      "logprobs": {...},
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 5,
    "total_tokens": 15
  }
}
```

---

## 関数インターフェース

### count_tokens

```cpp
/**
 * テキストのトークン数を計算
 * @param model llama_model ポインタ
 * @param text 計算対象テキスト
 * @return トークン数
 */
int count_tokens(const llama_model* model, const std::string& text);
```

### generate_response_id

```cpp
/**
 * 一意のレスポンスIDを生成
 * @param prefix プレフィックス（"chatcmpl" or "cmpl"）
 * @return 生成されたID
 */
std::string generate_response_id(const std::string& prefix);
```

### compute_logprobs

```cpp
/**
 * トークンのlog確率を計算
 * @param ctx llama_context ポインタ
 * @param tokens 生成されたトークン列
 * @param top_logprobs 上位候補数
 * @return LogprobInfo のベクタ
 */
std::vector<LogprobInfo> compute_logprobs(
    llama_context* ctx,
    const std::vector<llama_token>& tokens,
    int top_logprobs
);
```

### get_current_timestamp

```cpp
/**
 * 現在のUNIXタイムスタンプを取得
 * @return UNIXタイムスタンプ（秒）
 */
int64_t get_current_timestamp();
```

---

## Open Responses API関連（2026-01-16追加）

### Endpoint（拡張）

既存の`Endpoint`構造体（Rust）に以下のフィールドを追加:

```rust
pub struct Endpoint {
    // 既存フィールド...
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    pub status: EndpointStatus,
    // ...

    // 新規追加フィールド
    pub supports_responses_api: bool,  // Responses API対応フラグ
}
```

**検証ルール**:

- ヘルスチェック時に自動検出（OPTIONS /v1/responses → 200）
- 手動設定も可能

### SupportedAPI（新規）

モデルがサポートするAPI種別を表す列挙型:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportedAPI {
    ChatCompletions,
    Responses,
    Embeddings,
}
```

### EndpointModel（拡張）

既存の`EndpointModel`構造体に以下のフィールドを追加:

```rust
pub struct EndpointModel {
    pub endpoint_id: Uuid,
    pub model_id: String,
    pub capabilities: Option<Vec<String>>,
    pub last_checked: Option<DateTime<Utc>>,

    // 新規追加フィールド
    pub supported_apis: Vec<SupportedAPI>,  // サポートするAPI一覧
}
```

### データベーススキーマ変更

```sql
-- endpoints テーブル拡張
ALTER TABLE endpoints ADD COLUMN supports_responses_api BOOLEAN DEFAULT FALSE;

-- endpoint_models テーブル拡張
ALTER TABLE endpoint_models ADD COLUMN supported_apis TEXT DEFAULT '["chat_completions"]';
```

**注**: `supported_apis`はJSON配列として格納（SQLite互換）

### /v1/models レスポンス形式

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama3.2",
      "object": "model",
      "created": 1704067200,
      "owned_by": "ollama",
      "supported_apis": ["chat_completions", "responses"]
    },
    {
      "id": "gpt-4-turbo",
      "object": "model",
      "created": 1704067200,
      "owned_by": "openrouter",
      "supported_apis": ["chat_completions", "responses"]
    }
  ]
}
```

### 関連エンティティ図

```text
┌─────────────────────────────────────┐
│ Endpoint                            │
├─────────────────────────────────────┤
│ id: UUID                            │
│ name: String                        │
│ base_url: String                    │
│ status: EndpointStatus              │
│ supports_responses_api: bool  [NEW] │
│ ...                                 │
└─────────────────────────────────────┘
           │
           │ 1:N
           ▼
┌─────────────────────────────────────┐
│ EndpointModel                       │
├─────────────────────────────────────┤
│ endpoint_id: UUID                   │
│ model_id: String                    │
│ capabilities: Vec<String>           │
│ supported_apis: Vec<SupportedAPI>   │  [NEW]
│ ...                                 │
└─────────────────────────────────────┘
```
