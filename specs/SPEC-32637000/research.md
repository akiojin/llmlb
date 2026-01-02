# リサーチ: モデル capabilities ルーティング検証

## 調査目的

OpenAI互換APIでモデル指定時に、そのモデルが要求されたAPI機能に対応しているかを検証する仕組みを調査する。

## OpenAI モデル capabilities 参考

### OpenAI公式のモデル機能分類

| モデル | Text | Vision | Audio | Image Gen | Embedding |
|--------|------|--------|-------|-----------|-----------|
| gpt-4o | Yes | Yes | Yes | No | No |
| gpt-4-vision | Yes | Yes | No | No | No |
| whisper-1 | No | No | STT | No | No |
| tts-1 | No | No | TTS | No | No |
| dall-e-3 | No | No | No | Yes | No |
| text-embedding-3 | No | No | No | No | Yes |

## capabilities 設計

### enum定義

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    TextGeneration,   // /v1/chat/completions, /v1/completions
    TextToSpeech,     // /v1/audio/speech
    SpeechToText,     // /v1/audio/transcriptions
    ImageGeneration,  // /v1/images/generations
    Vision,           // /v1/chat/completions with images
    Embedding,        // /v1/embeddings
}
```

### APIエンドポイントとの対応

| エンドポイント | 必要なCapability |
|---------------|-----------------|
| /v1/chat/completions | TextGeneration |
| /v1/chat/completions (with images) | Vision |
| /v1/completions | TextGeneration |
| /v1/audio/speech | TextToSpeech |
| /v1/audio/transcriptions | SpeechToText |
| /v1/images/generations | ImageGeneration |
| /v1/embeddings | Embedding |

## 後方互換性

### ModelTypeからの自動推定

capabilities未設定のレガシーモデルに対応するため、ModelTypeから自動推定する。

```rust
impl ModelCapability {
    pub fn from_model_type(model_type: ModelType) -> Vec<Self> {
        match model_type {
            ModelType::Llm => vec![Self::TextGeneration],
            ModelType::Embedding => vec![Self::Embedding],
            ModelType::Tts => vec![Self::TextToSpeech],
            ModelType::Asr => vec![Self::SpeechToText],
            ModelType::ImageGeneration => vec![Self::ImageGeneration],
            ModelType::VisionLanguage => vec![
                Self::TextGeneration,
                Self::Vision,
            ],
        }
    }
}
```

## 検証フロー

### リクエスト処理

```text
[APIリクエスト]
     |
     v
[モデル名からModelInfo取得]
     |
     v
[capabilities取得（未設定ならModelTypeから推定）]
     |
     v
[必要なcapabilityがあるか確認]
     |
     +-- Yes --> [ノード選択・プロキシ]
     |
     +-- No --> [400 Bad Request エラー]
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

## 実装方針

### 検証関数

```rust
fn validate_capability(
    model_info: &ModelInfo,
    required: ModelCapability,
) -> Result<(), ApiError> {
    let capabilities = model_info.capabilities
        .clone()
        .unwrap_or_else(|| ModelCapability::from_model_type(model_info.model_type));

    if capabilities.contains(&required) {
        Ok(())
    } else {
        Err(ApiError::ModelCapabilityMismatch {
            model: model_info.name.clone(),
            capability: required,
        })
    }
}
```

### 各ハンドラーでの呼び出し

```rust
// audio.rs - TTS
validate_capability(&model_info, ModelCapability::TextToSpeech)?;

// openai.rs - Chat
validate_capability(&model_info, ModelCapability::TextGeneration)?;

// images.rs - Image Generation
validate_capability(&model_info, ModelCapability::ImageGeneration)?;
```

## 参考資料

- [OpenAI Models](https://platform.openai.com/docs/models)
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
