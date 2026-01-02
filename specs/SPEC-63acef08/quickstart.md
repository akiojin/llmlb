# クイックスタート: 統一APIプロキシ

## 前提条件

| 項目 | 要件 |
|------|------|
| ルーター | ビルド済み（Rust） |
| ノード | 1台以上のオンラインノード |
| APIキー | 有効なAPIキー |

## 基本設定

### 環境変数

```bash
# プロキシ設定
export LLM_ROUTER_REQUEST_TIMEOUT=60    # リクエストタイムアウト（秒）
export LLM_ROUTER_CONNECT_TIMEOUT=5     # 接続タイムアウト（秒）
export LLM_ROUTER_MAX_CONCURRENT=100    # 最大同時リクエスト数
```

## OpenAI互換API

### チャット完了（通常）

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello, how are you?"}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

**レスポンス例**:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1704067200,
  "model": "llama-3.2-1b",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I'm doing well, thank you for asking."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 12,
    "total_tokens": 37
  }
}
```

### チャット完了（ストリーミング）

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Tell me a joke"}],
    "stream": true
  }'
```

**レスポンス例**（SSE形式）:

```text
data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704067200,"model":"llama-3.2-1b","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704067200,"model":"llama-3.2-1b","choices":[{"index":0,"delta":{"content":"Why"},"finish_reason":null}]}

data: {"id":"chatcmpl-abc123","object":"chat.completion.chunk","created":1704067200,"model":"llama-3.2-1b","choices":[{"index":0,"delta":{"content":" don't"},"finish_reason":null}]}

data: [DONE]
```

### テキスト生成（Completions）

```bash
curl -X POST http://localhost:8080/v1/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "prompt": "The capital of France is",
    "max_tokens": 10
  }'
```

### Embeddings

```bash
curl -X POST http://localhost:8080/v1/embeddings \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "all-minilm-l6-v2",
    "input": "Hello, world!"
  }'
```

### モデル一覧

```bash
curl -X GET http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk-your-api-key"
```

## Python での利用

### OpenAI SDK互換

```python
from openai import OpenAI

# ルーターをエンドポイントに設定
client = OpenAI(
    api_key="sk-your-api-key",
    base_url="http://localhost:8080/v1"
)

# 通常のリクエスト
response = client.chat.completions.create(
    model="llama-3.2-1b",
    messages=[
        {"role": "user", "content": "What is the meaning of life?"}
    ]
)

print(response.choices[0].message.content)
```

### ストリーミング

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-your-api-key",
    base_url="http://localhost:8080/v1"
)

# ストリーミングリクエスト
stream = client.chat.completions.create(
    model="llama-3.2-1b",
    messages=[{"role": "user", "content": "Write a haiku"}],
    stream=True
)

for chunk in stream:
    if chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
print()
```

### Embeddings

```python
from openai import OpenAI

client = OpenAI(
    api_key="sk-your-api-key",
    base_url="http://localhost:8080/v1"
)

response = client.embeddings.create(
    model="all-minilm-l6-v2",
    input=["Hello world", "Goodbye world"]
)

for i, embedding in enumerate(response.data):
    print(f"Text {i}: {len(embedding.embedding)} dimensions")
```

### httpx での直接利用

```python
import httpx

def chat(prompt: str, stream: bool = False):
    with httpx.Client(timeout=60.0) as client:
        response = client.post(
            "http://localhost:8080/v1/chat/completions",
            headers={"Authorization": "Bearer sk-your-api-key"},
            json={
                "model": "llama-3.2-1b",
                "messages": [{"role": "user", "content": prompt}],
                "stream": stream
            }
        )

        if stream:
            for line in response.iter_lines():
                if line.startswith("data: ") and line != "data: [DONE]":
                    yield line[6:]
        else:
            return response.json()
```

## エラーハンドリング

### 利用可能なノードがない（503）

```json
{
  "error": {
    "message": "No available nodes",
    "type": "service_unavailable",
    "code": "no_nodes"
  }
}
```

**対処法**: ノードを起動し、オンライン状態を確認

### タイムアウト（504）

```json
{
  "error": {
    "message": "Request timeout after 60 seconds",
    "type": "gateway_timeout",
    "code": "timeout"
  }
}
```

**対処法**: `max_tokens` を減らすか、タイムアウト設定を延長

### 認証エラー（401）

```json
{
  "error": {
    "message": "Invalid API key",
    "type": "authentication_error",
    "code": "invalid_api_key"
  }
}
```

**対処法**: APIキーを確認

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Request History」でリクエスト履歴を確認

### 表示項目

| 項目 | 説明 |
|------|------|
| Model | 使用モデル |
| Node | 処理ノード |
| Duration | 処理時間 |
| Tokens | 使用トークン数 |
| Status | 成功/失敗 |

## パフォーマンス

| 指標 | 目標値 |
|------|--------|
| プロキシオーバーヘッド | <50ms |
| 同時リクエスト | 最大100 |
| ストリーミング遅延 | チャンク即時転送 |

## 制限事項

| 項目 | 制限 |
|------|------|
| リクエストタイムアウト | 60秒（デフォルト） |
| 同時リクエスト | 100（デフォルト） |
| フェイルオーバー | 未実装 |

## 次のステップ

- クラウドプロバイダー統合（`openai:gpt-4.1`など）
- 自動フェイルオーバーの実装
- リクエストキューイング
