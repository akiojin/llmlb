# クイックスタート: クラウドモデルプレフィックスルーティング

## 前提条件

| 項目 | 要件 |
|------|------|
| ロードバランサー | 起動済み（`http://localhost:8080`） |
| APIキー | 使用するクラウドプロバイダーのAPIキー |

## 環境変数の設定

```bash
# OpenAI（必須: gpt-4.1, gpt-4o等を使用する場合）
export OPENAI_API_KEY="sk-..."
export OPENAI_BASE_URL="https://api.openai.com/v1"  # オプション

# Google AI（必須: gemini-1.5-pro等を使用する場合）
export GOOGLE_API_KEY="..."
export GOOGLE_API_BASE_URL="https://generativelanguage.googleapis.com/v1beta"  # オプション

# Anthropic（必須: claude-3-opus等を使用する場合）
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_API_BASE_URL="https://api.anthropic.com/v1"  # オプション
```

## 基本的な使用例

### OpenAI モデルの使用

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4.1",
    "messages": [
      {"role": "user", "content": "Hello, how are you?"}
    ]
  }'
```

### Google AI モデルの使用

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google:gemini-1.5-pro",
    "messages": [
      {"role": "user", "content": "Explain quantum computing briefly."}
    ]
  }'
```

### Anthropic モデルの使用

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic:claude-3-opus",
    "messages": [
      {"role": "user", "content": "Write a haiku about programming."}
    ]
  }'
```

### ストリーミングレスポンス

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -N \
  -d '{
    "model": "openai:gpt-4.1",
    "messages": [
      {"role": "user", "content": "Tell me a story."}
    ],
    "stream": true
  }'
```

### Python での使用

```python
import httpx

BASE_URL = "http://localhost:8080"
HEADERS = {"Authorization": "Bearer sk-your-api-key"}

# OpenAI経由
response = httpx.post(
    f"{BASE_URL}/v1/chat/completions",
    headers=HEADERS,
    json={
        "model": "openai:gpt-4.1",
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
print(response.json()["choices"][0]["message"]["content"])

# ストリーミング
with httpx.stream(
    "POST",
    f"{BASE_URL}/v1/chat/completions",
    headers=HEADERS,
    json={
        "model": "anthropic:claude-3-opus",
        "messages": [{"role": "user", "content": "Tell me a joke."}],
        "stream": True
    }
) as response:
    for line in response.iter_lines():
        if line.startswith("data: "):
            data = line[6:]
            if data != "[DONE]":
                import json
                chunk = json.loads(data)
                content = chunk["choices"][0]["delta"].get("content", "")
                print(content, end="", flush=True)
```

### OpenAI Python SDKとの互換性

```python
from openai import OpenAI

# ロードバランサーをベースURLとして指定
client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk-your-api-key"
)

# クラウドモデルを使用
response = client.chat.completions.create(
    model="openai:gpt-4.1",  # プレフィックス付き
    messages=[{"role": "user", "content": "Hello!"}]
)

print(response.choices[0].message.content)
```

## ローカルモデルとの使い分け

```python
# ローカルモデル（プレフィックスなし）
local_response = httpx.post(
    f"{BASE_URL}/v1/chat/completions",
    headers=HEADERS,
    json={
        "model": "llama-3.2-1b",  # プレフィックスなし → ローカル
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)

# クラウドモデル（プレフィックスあり）
cloud_response = httpx.post(
    f"{BASE_URL}/v1/chat/completions",
    headers=HEADERS,
    json={
        "model": "openai:gpt-4.1",  # プレフィックスあり → クラウド
        "messages": [{"role": "user", "content": "Hello!"}]
    }
)
```

## プロバイダー状態の確認

```bash
# ロードバランサーのステータスを確認
curl http://localhost:8080/v0/status \
  -H "Authorization: Bearer sk-your-api-key"
```

### レスポンス例

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

## エラーハンドリング

### APIキー未設定

```bash
# ANTHROPIC_API_KEYが未設定の場合
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic:claude-3-opus",
    "messages": [{"role": "user", "content": "Hello"}]
  }'

# HTTP 401 Unauthorized
{
  "error": {
    "message": "Anthropic API key not configured",
    "type": "authentication_error",
    "code": "api_key_missing"
  }
}
```

### 不明なプレフィックス

```bash
# 存在しないプロバイダー
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "unknown:some-model",
    "messages": [{"role": "user", "content": "Hello"}]
  }'

# HTTP 400 Bad Request
{
  "error": {
    "message": "Unknown cloud provider prefix: unknown:",
    "type": "invalid_request_error",
    "code": "unknown_prefix"
  }
}
```

### レート制限

```bash
# HTTP 429 Too Many Requests
{
  "error": {
    "message": "Rate limit exceeded. Please retry after 60 seconds.",
    "type": "rate_limit_error",
    "code": "rate_limited"
  }
}
```

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Settings」→「Cloud Providers」で設定状態を確認

| 表示項目 | 説明 |
|---------|------|
| OpenAI | 設定済み/未設定 |
| Google | 設定済み/未設定 |
| Anthropic | 設定済み/未設定 |

## 制限事項

| 項目 | 制限 |
|------|------|
| 対応プロバイダー | OpenAI, Google, Anthropic のみ |
| 自動フォールバック | 非対応（明示的プレフィックス必須） |
| レート制限リトライ | 非対応（429をそのまま返却） |
| 認証方式 | APIキーのみ（OAuth非対応） |
| カスタムプロバイダー | 非対応 |

## 重要なルール

1. **プレフィックス必須**: クラウドモデルを使用するには必ずプレフィックスを指定
2. **自動ルーティングなし**: `gpt-4o` のように OpenAI モデル名をそのまま指定しても、
   ローカルノードへルーティングされる（クラウドには送信されない）
3. **コスト管理**: プレフィックス必須により、予期しないクラウド課金を防止

## 次のステップ

- 複数プロバイダーの並列利用
- 使用量モニタリングの設定
- 本番環境でのシークレット管理（Kubernetes Secrets等）
