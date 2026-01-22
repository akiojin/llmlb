# クイックスタート: Open Responses API

**機能ID**: `SPEC-24157000` | **日付**: 2026-01-16

## 前提条件

- llmlbが起動している
- Responses API対応バックエンド（Ollama v0.13.3+, vLLM等）が登録されている
- 有効なAPIキーを持っている

## 基本的な使い方

### 1. 対応モデルの確認

```bash
curl -H "Authorization: Bearer YOUR_API_KEY" \
  http://localhost:3000/v1/models | jq '.data[] | select(.supported_apis | contains(["responses"]))'
```

### 2. 基本リクエスト（非ストリーミング）

```bash
curl -X POST http://localhost:3000/v1/responses \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2",
    "input": "Hello, how are you?",
    "stream": false
  }'
```

### 3. ストリーミングリクエスト

```bash
curl -X POST http://localhost:3000/v1/responses \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.2",
    "input": "Write a short poem about AI",
    "stream": true
  }'
```

## テストシナリオ

### US6: 基本リクエスト

```bash
# 期待: 200 OK + バックエンドからのレスポンス
curl -X POST http://localhost:3000/v1/responses \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama3.2", "input": "Hello"}'
```

### US7: ストリーミング

```bash
# 期待: Server-Sent Eventsストリーム
curl -X POST http://localhost:3000/v1/responses \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama3.2", "input": "Count to 5", "stream": true}'
```

### US8: 非対応バックエンド

```bash
# 期待: 501 Not Implemented
curl -X POST http://localhost:3000/v1/responses \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "non-responses-model", "input": "Hello"}'
```

### US9: 対応確認

```bash
# 期待: supported_apisフィールドを含むモデル一覧
curl -H "Authorization: Bearer YOUR_API_KEY" \
  http://localhost:3000/v1/models
```

## エラーハンドリング

| ステータス | 意味 | 対処 |
|-----------|------|------|
| 200 | 成功 | - |
| 401 | 認証エラー | APIキーを確認 |
| 501 | 非対応バックエンド | /v1/modelsで対応モデルを確認 |
| 502 | バックエンドエラー | バックエンドの状態を確認 |
| 503 | サービス利用不可 | バックエンドの起動を待つ |

## 既存APIとの比較

| 機能 | Chat Completions | Responses API |
|------|------------------|---------------|
| エンドポイント | `/v1/chat/completions` | `/v1/responses` |
| メッセージ形式 | `messages: [{role, content}]` | `input: string` |
| ストリーミング | SSE | SSE |
| ステートフル | なし | `previous_response_id` |
| ツール | `tools` | `tools` (extended) |

## 注意事項

- ルーターはパススルーのみ。API変換は行わない
- ステートフル機能（previous_response_id）はバックエンドの責務
- ツール実行はクライアントの責務
