# API契約: Open Responses API

**機能ID**: `SPEC-24157000` | **日付**: 2026-01-16

## POST /v1/responses

Responses APIリクエストをバックエンドにパススルー。

### リクエスト

**Headers**:

```text
Authorization: Bearer {api_key}
Content-Type: application/json
```

**Body**: Open Responses API仕様準拠（パススルー）

```json
{
  "model": "string",
  "input": "string | array",
  "instructions": "string (optional)",
  "stream": "boolean (optional, default: false)",
  "previous_response_id": "string (optional)",
  "tools": "array (optional)",
  "tool_choice": "string | object (optional)",
  "temperature": "number (optional)",
  "max_output_tokens": "integer (optional)"
}
```

### レスポンス

**成功 (200)**:

バックエンドからの生レスポンスをそのまま返却。

```json
{
  "id": "resp_xxx",
  "object": "response",
  "created_at": 1704067200,
  "model": "llama3.2",
  "output": [
    {
      "type": "message",
      "role": "assistant",
      "content": [
        {
          "type": "output_text",
          "text": "Hello! How can I help you?"
        }
      ]
    }
  ],
  "usage": {
    "input_tokens": 10,
    "output_tokens": 8,
    "total_tokens": 18
  }
}
```

**ストリーミング (200, stream=true)**:

Server-Sent Eventsとしてバックエンドからのイベントをそのまま転送。

```text
event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"Hello"}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"!"}

event: response.completed
data: {"type":"response.completed","response":{...}}
```

**認証エラー (401)**:

```json
{
  "error": "Unauthorized: Invalid API key"
}
```

**非対応バックエンド (501)**:

指定されたモデルを提供するバックエンドがResponses APIに対応していない場合。

```json
{
  "error": "Not Implemented: The backend for model 'xxx' does not support Responses API"
}
```

**バックエンドエラー (502)**:

バックエンドからのエラーレスポンスをそのまま転送。

```json
{
  "error": {
    "message": "Backend error message",
    "type": "backend_error",
    "code": "xxx"
  }
}
```

**サービス利用不可 (503)**:

利用可能なバックエンドがない場合。

```json
{
  "error": "Service Unavailable: No available backends"
}
```

## GET /v1/models (拡張)

既存のモデル一覧APIに`supported_apis`フィールドを追加。

### レスポンス

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
      "id": "mistral-7b",
      "object": "model",
      "created": 1704067200,
      "owned_by": "vllm",
      "supported_apis": ["chat_completions"]
    }
  ]
}
```

## エラーコード一覧

| ステータス | エラーコード | 説明 |
|-----------|-------------|------|
| 401 | unauthorized | 認証エラー |
| 501 | not_implemented | バックエンドがResponses API非対応 |
| 502 | bad_gateway | バックエンドエラー（転送） |
| 503 | service_unavailable | 利用可能なバックエンドなし |
| 504 | gateway_timeout | バックエンドタイムアウト |
