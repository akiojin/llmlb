# データモデル: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000` | **日付**: 2024-12-14

## エンティティ定義

### 1. ModelType (拡張)

**ファイル**: `common/src/types.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    /// 言語モデル（デフォルト）
    #[default]
    Llm,
    /// Embeddingモデル
    Embedding,
    /// 音声認識モデル (ASR)
    #[serde(rename = "speech_to_text")]
    SpeechToText,
    /// 音声合成モデル (TTS)
    #[serde(rename = "text_to_speech")]
    TextToSpeech,
}
```

**検証ルール**:

- デフォルト値は `Llm`
- シリアライズ時は小文字スネークケース

### 2. RuntimeType (新規)

**ファイル**: `common/src/types.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeType {
    /// llama.cpp (テキスト生成、Embedding)
    #[default]
    #[serde(rename = "llama_cpp")]
    LlamaCpp,
    /// Audio ASR engine (safetensors)
    #[serde(rename = "audio_asr")]
    AudioAsr,
    /// Audio TTS engine (safetensors)
    #[serde(rename = "audio_tts")]
    AudioTts,
}
```

**ModelTypeとの対応**:

| ModelType | RuntimeType |
|-----------|-------------|
| Llm | LlamaCpp |
| Embedding | LlamaCpp |
| SpeechToText | AudioAsr |
| TextToSpeech | AudioTts |

### 3. RequestType (拡張)

**ファイル**: `common/src/protocol.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestType {
    /// /v1/chat/completions
    Chat,
    /// /v1/completions
    Generate,
    /// /v1/embeddings
    Embeddings,
    /// /v1/audio/transcriptions (新規)
    Transcription,
    /// /v1/audio/speech (新規)
    Speech,
}
```

### 4. AudioFormat (新規)

**ファイル**: `common/src/types.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV (PCM)
    Wav,
    /// MP3
    Mp3,
    /// FLAC (ロスレス)
    Flac,
    /// Ogg Vorbis
    Ogg,
    /// Opus
    Opus,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self::Mp3
    }
}
```

### 5. TranscriptionRequest (新規)

**ファイル**: `common/src/protocol.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    /// モデル名 (例: "whisper-large-v3")
    pub model: String,
    /// 音声ファイルデータ (multipartから取得)
    #[serde(skip)]
    pub file_data: Vec<u8>,
    /// 元のファイル名
    #[serde(skip)]
    pub filename: String,
    /// 言語コード (ISO 639-1)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// レスポンス形式
    #[serde(default)]
    pub response_format: TranscriptionResponseFormat,
    /// タイムスタンプの粒度
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionResponseFormat {
    #[default]
    Json,
    Text,
    Srt,
    Vtt,
    VerboseJson,
}
```

### 6. TranscriptionResponse (新規)

**ファイル**: `common/src/protocol.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    /// 認識されたテキスト
    pub text: String,
    /// 検出された言語 (verbose_json時)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 処理時間 (秒)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,
    /// セグメント (verbose_json時)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    pub id: u32,
    pub start: f32,
    pub end: f32,
    pub text: String,
}
```

### 7. SpeechRequest (新規)

**ファイル**: `common/src/protocol.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechRequest {
    /// モデル名 (例: "vibevoice-v1", "tts-1")
    pub model: String,
    /// 読み上げるテキスト
    pub input: String,
    /// 音声タイプ (voice)
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 出力フォーマット
    #[serde(default)]
    pub response_format: AudioFormat,
    /// 読み上げ速度 (0.25〜4.0)
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_voice() -> String {
    "nova".to_string()
}

fn default_speed() -> f32 {
    1.0
}
```

### 8. Node拡張

**ファイル**: `common/src/types.rs`

```rust
pub struct Node {
    // 既存フィールド...

    /// ロード済みASRモデル一覧 (新規)
    #[serde(default)]
    pub loaded_asr_models: Vec<String>,

    /// ロード済みTTSモデル一覧 (新規)
    #[serde(default)]
    pub loaded_tts_models: Vec<String>,

    /// サポートするランタイム一覧 (新規)
    #[serde(default)]
    pub supported_runtimes: Vec<RuntimeType>,
}
```

### 9. HealthCheckRequest拡張

**ファイル**: `common/src/protocol.rs`

```rust
pub struct HealthCheckRequest {
    // 既存フィールド...

    /// ロード済みASRモデル一覧 (新規)
    #[serde(default)]
    pub loaded_asr_models: Vec<String>,

    /// ロード済みTTSモデル一覧 (新規)
    #[serde(default)]
    pub loaded_tts_models: Vec<String>,

    /// サポートするランタイム一覧 (新規)
    #[serde(default)]
    pub supported_runtimes: Vec<RuntimeType>,
}
```

## 関係図

```text
┌─────────────────┐
│   ModelType     │
├─────────────────┤
│ Llm             │──┐
│ Embedding       │──┤
│ SpeechToText    │──┼──→ RuntimeType
│ TextToSpeech    │──┘
└─────────────────┘

┌─────────────────┐      ┌─────────────────┐
│   RequestType   │      │   AudioFormat   │
├─────────────────┤      ├─────────────────┤
│ Chat            │      │ Wav             │
│ Generate        │      │ Mp3             │
│ Embeddings      │      │ Flac            │
│ Transcription   │──────│ Ogg             │
│ Speech          │──────│ Opus            │
└─────────────────┘      └─────────────────┘

┌─────────────────┐      ┌─────────────────┐
│ Transcription   │      │   Speech        │
│ Request         │      │   Request       │
├─────────────────┤      ├─────────────────┤
│ model           │      │ model           │
│ file_data       │      │ input           │
│ language?       │      │ voice           │
│ response_format │      │ response_format │
└─────────────────┘      │ speed           │
                         └─────────────────┘
```

## 状態遷移

音声リクエストの状態遷移は既存の `RequestStatus` をそのまま使用:

```text
Pending → Processing → Completed
                    ↘ Failed
```

## 検証ルール

| フィールド | ルール |
|-----------|--------|
| SpeechRequest.input | 1〜4096文字 |
| SpeechRequest.speed | 0.25〜4.0 |
| TranscriptionRequest.file_data | 25MB以下 |
| TranscriptionRequest.language | ISO 639-1 コード |
