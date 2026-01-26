# クイックスタート: xLLM Responses API

**機能ID**: `SPEC-1a74455c`

## 前提

- xLLMが起動している
- モデルがロード可能である

## 基本リクエスト

```bash
curl -X POST http://localhost:32769/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-oss-7b","input":"Hello"}'
```

## ストリーミング

```bash
curl -N -X POST http://localhost:32769/v1/responses \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-oss-7b","input":"Hello","stream":true}'
```

## ヘルスチェック

```bash
curl http://localhost:32769/health
```

`supports_responses_api: true`が含まれることを確認する。
