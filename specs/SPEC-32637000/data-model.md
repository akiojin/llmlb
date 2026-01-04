# データモデル: モデル capabilities ルーティング検証

## エンティティ定義

### ModelCapability

モデルが対応するAPI機能を表現するenum。

```rust
/// モデルが対応するAPI機能
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    /// テキスト生成 (/v1/chat/completions, /v1/completions)
    TextGeneration,
    /// 音声合成 (/v1/audio/speech)
    TextToSpeech,
    /// 音声認識 (/v1/audio/transcriptions)
    SpeechToText,
    /// 画像生成 (/v1/images/generations)
    ImageGeneration,
    /// 画像理解 (/v1/chat/completions with images)
    Vision,
    /// 埋め込み生成 (/v1/embeddings)
    Embedding,
}
```

### ModelInfo拡張

既存のModelInfoにcapabilitiesフィールドを追加。

```rust
pub struct ModelInfo {
    /// モデルID（一意識別子）
    pub id: Uuid,
    /// モデル名
    pub name: String,
    /// モデルタイプ（LLM, Embedding, TTS等）
    pub model_type: ModelType,
    /// 対応するAPI機能（Noneの場合はmodel_typeから推定）
    pub capabilities: Option<Vec<ModelCapability>>,
    /// ノードID（このモデルをホストするノード）
    pub node_id: Uuid,
    /// 作成日時
    pub created_at: DateTime<Utc>,
}
```

## ModelTypeからの推定ルール

| ModelType | 推定されるCapabilities |
|-----------|----------------------|
| Llm | [TextGeneration] |
| Embedding | [Embedding] |
| Tts | [TextToSpeech] |
| Asr | [SpeechToText] |
| ImageGeneration | [ImageGeneration] |
| VisionLanguage | [TextGeneration, Vision] |

## API/Capabilityマッピング

| エンドポイント | メソッド | 必要なCapability |
|---------------|---------|-----------------|
| /v1/chat/completions | POST | TextGeneration |
| /v1/completions | POST | TextGeneration |
| /v1/audio/speech | POST | TextToSpeech |
| /v1/audio/transcriptions | POST | SpeechToText |
| /v1/images/generations | POST | ImageGeneration |
| /v1/embeddings | POST | Embedding |

### 画像付きチャットの判定

```rust
fn requires_vision(request: &ChatCompletionsRequest) -> bool {
    request.messages.iter().any(|msg| {
        msg.content.iter().any(|c| matches!(c, ContentPart::ImageUrl { .. }))
    })
}

// 使用例
let required = if requires_vision(&request) {
    ModelCapability::Vision
} else {
    ModelCapability::TextGeneration
};
```

## エラーモデル

### ModelCapabilityMismatchError

```rust
pub struct ModelCapabilityMismatchError {
    /// モデル名
    pub model: String,
    /// 要求されたが持っていないcapability
    pub required_capability: ModelCapability,
    /// モデルが持っているcapabilities
    pub available_capabilities: Vec<ModelCapability>,
}
```

### エラーレスポンスJSON

```json
{
  "error": {
    "message": "Model 'llama-3.1-8b' does not support text-to-speech",
    "type": "invalid_request_error",
    "code": "model_capability_mismatch"
  }
}
```

## /v1/modelsレスポンス拡張

### 従来のレスポンス

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3.1-8b",
      "object": "model",
      "created": 1704067200,
      "owned_by": "local"
    }
  ]
}
```

### 拡張後のレスポンス

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3.1-8b",
      "object": "model",
      "created": 1704067200,
      "owned_by": "local",
      "capabilities": ["text_generation"]
    },
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1704067200,
      "owned_by": "openai",
      "capabilities": ["text_generation", "vision", "text_to_speech"]
    }
  ]
}
```

## シリアライゼーション

### JSON形式

```json
{
  "capabilities": ["text_generation", "vision"]
}
```

### Rust/Serde設定

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability { ... }
```
