# クイックスタート: モデル登録キャッシュとマルチモーダルI/O

## 概要

モデルの登録、チャット、画像・音声の入出力、削除の完全な動作フローを説明する。

## モデル登録とキャッシュ

### モデル登録

```bash
curl -X POST http://localhost:8080/v1/models \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model_id": "llama-3.1-8b",
    "source": "huggingface",
    "repo_id": "meta-llama/Llama-3.1-8B-GGUF"
  }'
```

### モデル一覧確認

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" | jq '.data[] | {id, ready}'
```

### レスポンス例

```json
[
  {"id": "llama-3.1-8b", "ready": true},
  {"id": "whisper-large-v3", "ready": true},
  {"id": "stable-diffusion-v2", "ready": false}
]
```

## テキスト生成（Chat Completions）

### 指定モデルでチャット

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "llama-3.1-8b",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## 画像生成（Image Generation）

### 画像生成

```bash
curl http://localhost:8080/v1/images/generations \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "stable-diffusion-v2",
    "prompt": "A beautiful sunset over mountains",
    "n": 1,
    "size": "512x512"
  }'
```

### レスポンス例

```json
{
  "created": 1704067200,
  "data": [
    {"url": "http://localhost:8080/v1/images/abc123.png"}
  ]
}
```

## 音声認識（Speech to Text）

### 音声ファイルの文字起こし

```bash
curl http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer sk_debug" \
  -F "model=whisper-large-v3" \
  -F "file=@audio.mp3"
```

### レスポンス例

```json
{
  "text": "Hello, this is a test recording."
}
```

## 音声合成（Text to Speech）

### テキストから音声生成

```bash
curl http://localhost:8080/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "vibevoice",
    "input": "こんにちは、世界！",
    "voice": "alloy"
  }' \
  --output speech.mp3
```

## モデル削除

### 指定モデルの削除

```bash
curl -X DELETE http://localhost:8080/v1/models/llama-3.1-8b \
  -H "Authorization: Bearer sk_debug"
```

### 削除確認

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" | jq '.data[].id'
# llama-3.1-8b が含まれていないことを確認
```

## エラーハンドリング

### 未登録モデルの使用

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{"model": "unknown-model", "messages": [{"role": "user", "content": "Hi"}]}'
```

### エラーレスポンス

```json
{
  "error": {
    "message": "Model 'unknown-model' not found",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

HTTPステータス: `404 Not Found`

### 非対応APIでのモデル使用

```bash
curl http://localhost:8080/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{"model": "llama-3.1-8b", "input": "Hello", "voice": "alloy"}'
```

### capability mismatchエラー

```json
{
  "error": {
    "message": "Model 'llama-3.1-8b' does not support text-to-speech",
    "type": "invalid_request_error",
    "code": "model_capability_mismatch"
  }
}
```

HTTPステータス: `400 Bad Request`

## トラブルシューティング

### モデルがreadyにならない

1. ノードのログを確認: ダウンロード進捗やエラーを確認
2. キャッシュファイルサイズ確認: 0Bの場合は破損
3. ノードの対応ランタイム確認: 必要なランタイムがサポートされているか

### 画像/音声APIが503を返す

1. 対応ランタイムを持つノードが登録されているか確認
2. ノードがヘルスチェックに応答しているか確認
3. モデルがロードされているか確認
