# クイックスタート: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d`
**作成日**: 2026-01-27

## 前提

- xllmがGPUバックエンドでビルドされていること
- safetensorsモデルを利用可能であること

## 手順

1. safetensorsモデルを取得する

```bash
xllm pull openai/gpt-oss-20b
```

1. KVキャッシュ量子化を指定して推論する（`kv_int8` / `kv_fp8`）

```bash
curl -sS http://127.0.0.1:32769/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai/gpt-oss-20b:kv_int8",
    "messages": [{"role": "user", "content": "Say hello briefly."}],
    "temperature": 0
  }'
```

1. 量子化状態を確認する

```bash
xllm show openai/gpt-oss-20b:kv_int8
xllm list
```

## 確認ポイント

- 量子化指定時に推論が成功する
- 量子化状態が一覧/詳細に表示される
- 未対応方式指定時に明確なエラーが返る
