# データモデル: xLLM Responses API

**機能ID**: `SPEC-1a74455c`

## リクエスト

### POST /v1/responses

- `model`: string (必須)
- `input`: string | array (必須)
- `instructions`: string (任意)
- `stream`: boolean (任意, default: false)
- `temperature`: number (任意)
- `max_output_tokens`: integer (任意)

### 入力の扱い

- `input`が文字列の場合は単一メッセージとして扱う
- `input`が配列の場合は、メッセージ配列または
  コンテンツ配列として扱う

## レスポンス

### 成功 (200)

- `id`: string (例: resp_...)
- `object`: "response"
- `created_at`: UNIX秒
- `model`: string
- `output`: 配列
  - `type`: "message"
  - `role`: "assistant"
  - `content`: 配列
    - `type`: "output_text"
    - `text`: 生成結果
- `usage`:
  - `input_tokens`: int
  - `output_tokens`: int
  - `total_tokens`: int

### ストリーミング (SSE)

- `event: response.output_text.delta`
- `event: response.completed`

## ヘルスチェック

- `/health`に`supports_responses_api: true`を含める
