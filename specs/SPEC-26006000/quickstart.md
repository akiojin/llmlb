# クイックスタート: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000` | **日付**: 2024-12-14

## 前提条件

- llm-router が起動していること
- 音声対応ノードが登録されていること
- 認証トークンを取得済みであること
- VibeVoice ランナー利用時は以下を設定していること
  - `LLM_NODE_VIBEVOICE_MODEL=microsoft/VibeVoice-1.5B`
  - `LLM_NODE_VIBEVOICE_VOICE_PROMPT=<音声プロンプトWAVのパス>`

## 音声認識 (ASR)

### 基本的な音声認識

```bash
# WAVファイルをテキストに変換
curl -X POST http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@audio.wav" \
  -F "model=whisper-large-v3"
```

**レスポンス**:

```json
{
  "text": "こんにちは、今日はいい天気ですね。"
}
```

### 言語指定付き音声認識

```bash
# 日本語を明示的に指定
curl -X POST http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@meeting.mp3" \
  -F "model=whisper-large-v3" \
  -F "language=ja"
```

### タイムスタンプ付き詳細出力

```bash
# verbose_json形式でタイムスタンプを取得
curl -X POST http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@podcast.mp3" \
  -F "model=whisper-large-v3" \
  -F "response_format=verbose_json"
```

**レスポンス**:

```json
{
  "text": "こんにちは、今日はいい天気ですね。",
  "language": "ja",
  "duration": 3.5,
  "segments": [
    {
      "id": 0,
      "start": 0.0,
      "end": 1.8,
      "text": "こんにちは、"
    },
    {
      "id": 1,
      "start": 1.8,
      "end": 3.5,
      "text": "今日はいい天気ですね。"
    }
  ]
}
```

### SRT字幕形式で出力

```bash
# SRT形式で字幕ファイルを生成
curl -X POST http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@video.mp3" \
  -F "model=whisper-large-v3" \
  -F "response_format=srt" \
  -o subtitles.srt
```

## 音声合成 (TTS)

### 基本的な音声合成

```bash
# テキストを音声に変換
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "vibevoice-v1",
    "input": "こんにちは、今日はいい天気ですね。",
    "response_format": "wav"
  }' \
  --output speech.wav
```

### 音声タイプを指定

```bash
# 女性音声で生成
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "vibevoice-v1",
    "input": "お知らせです。",
    "voice": "nova",
    "response_format": "wav"
  }' \
  --output announcement.wav
```

利用可能なvoice:

- `alloy` - ニュートラル
- `echo` - 男性的
- `fable` - ナレーション向け
- `onyx` - 深い男性的
- `nova` - 女性的 (デフォルト)
- `shimmer` - 明るい女性的

### 出力フォーマットを指定

```bash
# WAV形式で出力
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "vibevoice-v1",
    "input": "高品質な音声です。",
    "response_format": "wav"
  }' \
  --output speech.wav
```

> VibeVoice ランナーは現状 WAV 出力のみです。
> `response_format` に mp3/opus/flac を指定しても WAV で返るため、
> 必要なら別途変換してください。

### 読み上げ速度を調整

```bash
# 1.5倍速で読み上げ
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "vibevoice-v1",
    "input": "早口で読み上げます。",
    "speed": 1.5,
    "response_format": "wav"
  }' \
  --output fast_speech.wav
```

## Python SDKでの使用

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="your-token"
)

# 音声認識
with open("audio.wav", "rb") as f:
    transcript = client.audio.transcriptions.create(
        model="whisper-large-v3",
        file=f,
        language="ja"
    )
print(transcript.text)

# 音声合成
response = client.audio.speech.create(
    model="vibevoice-v1",
    input="こんにちは",
    voice="nova",
    response_format="wav"
)
response.stream_to_file("output.wav")
```

## エラーハンドリング

### サポートされていないフォーマット

```json
{
  "error": {
    "message": "Unsupported audio format: .aac",
    "type": "invalid_request_error",
    "code": "unsupported_format"
  }
}
```

### ファイルサイズ超過

```json
{
  "error": {
    "message": "File size exceeds limit of 25MB",
    "type": "invalid_request_error",
    "code": "file_too_large"
  }
}
```

### モデルが見つからない

```json
{
  "error": {
    "message": "Model 'whisper-tiny' not found",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

## 対応フォーマット

### 入力 (ASR)

| フォーマット | 拡張子 | MIMEタイプ |
|------------|--------|-----------|
| WAV | .wav | audio/wav |
| MP3 | .mp3 | audio/mpeg |
| FLAC | .flac | audio/flac |
| OGG | .ogg | audio/ogg |

### 出力 (TTS)

| フォーマット | 拡張子 | MIMEタイプ |
|------------|--------|-----------|
| MP3 | .mp3 | audio/mpeg |
| WAV | .wav | audio/wav |
| FLAC | .flac | audio/flac |
| Opus | .opus | audio/opus |

## 制限事項

| 項目 | 制限 |
|------|------|
| 音声ファイルサイズ | 最大25MB |
| TTSテキスト長 | 最大4096文字 |
| 同時処理数 | ノードあたり10件 |
| 対応言語 (ASR) | Whisperがサポートする99言語 |
