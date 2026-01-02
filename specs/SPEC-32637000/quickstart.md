# クイックスタート: モデル capabilities ルーティング検証

## 概要

モデルのcapabilitiesを確認し、適切なAPIエンドポイントで使用する方法を説明する。

## モデル一覧でcapabilitiesを確認

```bash
curl http://localhost:8080/v1/models | jq '.data[] | {id, capabilities}'
```

### レスポンス例

```json
[
  {
    "id": "llama-3.1-8b",
    "capabilities": ["text_generation"]
  },
  {
    "id": "whisper-large-v3",
    "capabilities": ["speech_to_text"]
  },
  {
    "id": "vibevoice",
    "capabilities": ["text_to_speech"]
  },
  {
    "id": "gpt-4o",
    "capabilities": ["text_generation", "vision", "text_to_speech"]
  }
]
```

## 正常なAPI呼び出し

### テキスト生成（TextGeneration対応モデル）

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "llama-3.1-8b",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### 音声合成（TextToSpeech対応モデル）

```bash
curl http://localhost:8080/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "vibevoice",
    "input": "Hello, world!",
    "voice": "alloy"
  }' \
  --output speech.mp3
```

### 音声認識（SpeechToText対応モデル）

```bash
curl http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer sk_debug" \
  -F "model=whisper-large-v3" \
  -F "file=@audio.mp3"
```

## 非対応モデルでのエラー

### 例: LLMモデルで音声合成を試みる

```bash
curl http://localhost:8080/v1/audio/speech \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "llama-3.1-8b",
    "input": "Hello, world!",
    "voice": "alloy"
  }'
```

### エラーレスポンス

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

## capability別のAPI対応表

| Capability | 対応API |
|------------|---------|
| text_generation | /v1/chat/completions, /v1/completions |
| text_to_speech | /v1/audio/speech |
| speech_to_text | /v1/audio/transcriptions |
| image_generation | /v1/images/generations |
| vision | /v1/chat/completions (画像付き) |
| embedding | /v1/embeddings |

## 画像付きチャット（Vision）

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "gpt-4o",
    "messages": [{
      "role": "user",
      "content": [
        {"type": "text", "text": "What is in this image?"},
        {"type": "image_url", "image_url": {"url": "https://example.com/image.jpg"}}
      ]
    }]
  }'
```

## 後方互換性

### capabilities未設定のモデル

capabilitiesフィールドが未設定のレガシーモデルは、ModelTypeから自動推定される。

| ModelType | 自動推定されるCapabilities |
|-----------|--------------------------|
| llm | text_generation |
| embedding | embedding |
| tts | text_to_speech |
| asr | speech_to_text |
| image_generation | image_generation |
| vision_language | text_generation, vision |

## トラブルシューティング

### "model_capability_mismatch"エラー

1. `/v1/models`でモデルのcapabilitiesを確認
2. 使用したいAPIに対応するcapabilityを持つモデルを選択
3. 正しいモデル名でリクエストを再送信

### モデルが見つからない

```json
{
  "error": {
    "message": "Model 'unknown-model' not found",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

- `/v1/models`で利用可能なモデル一覧を確認
