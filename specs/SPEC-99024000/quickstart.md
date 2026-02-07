# クイックスタート: 機能仕様書: Open Responses API対応

## 前提条件

| 項目 | 要件 |
|------|------|
| API | `/v1/responses` が有効（Responses API対応バックエンドが登録済み） |
| 認証 | APIキー（`Authorization: Bearer sk-...`） |
| モデル | `/v1/models` で Responses 対応を確認 |

## 基本的な使用例

### レスポンス生成（非ストリーミング）

```bash
curl -X POST http://localhost:8080/v1/responses \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "your-model",
    "input": "Hello, Responses API"
  }'
```

### ストリーミングでの応答

```bash
curl -N -X POST http://localhost:8080/v1/responses \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "your-model",
    "input": "Stream please",
    "stream": true
  }'
```

## 対応状況の確認

Responses APIの対応状況は `/v1/models` の返却内容で確認します。

```bash
curl -X GET http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk-your-api-key"
```

## エラーハンドリング

- **HTTP 501**: Responses API対応バックエンドが存在しない
- **HTTP 401**: APIキーが無効

## 参照

- spec.md
- plan.md
