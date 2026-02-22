//! モデル関連型定義
//!
//! モデルタイプ、ランタイムタイプ、モデル能力などの定義

use serde::{Deserialize, Serialize};

/// モデル同期状態
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    /// 同期待機
    Idle,
    /// 同期中
    Running,
    /// 同期成功
    Success,
    /// 同期失敗
    Failed,
}

/// モデル同期の進捗情報
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncProgress {
    /// 対象モデルID
    pub model_id: String,
    /// 対象ファイル名
    pub file: String,
    /// ダウンロード済みバイト数
    pub downloaded_bytes: u64,
    /// 総バイト数
    pub total_bytes: u64,
}

/// モデルタイプ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    /// 言語モデル（デフォルト）
    #[default]
    Llm,
    /// Embeddingモデル
    Embedding,
    /// 音声認識モデル (ASR: Speech-to-Text)
    #[serde(rename = "speech_to_text")]
    SpeechToText,
    /// 音声合成モデル (TTS: Text-to-Speech)
    #[serde(rename = "text_to_speech")]
    TextToSpeech,
    /// 画像生成モデル (Text-to-Image)
    #[serde(rename = "image_generation")]
    ImageGeneration,
}

/// ランタイムタイプ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    /// llama.cpp (テキスト生成、Embedding)
    #[default]
    LlamaCpp,
    /// safetensors-cpp ベースの Nemotron 直接ロード
    NemotronCpp,
    /// OpenAI gpt-oss 公式ランタイム（Metal/CUDA などの最適化アーティファクト）
    #[serde(rename = "gptoss_cpp")]
    GptOssCpp,
    /// safetensors.cpp (safetensors形式モデルの直接ロード)
    SafetensorsCpp,
    /// whisper.cpp (音声認識)
    WhisperCpp,
    /// ONNX Runtime (TTS、汎用推論)
    OnnxRuntime,
    /// stable-diffusion.cpp (画像生成)
    StableDiffusion,
}

/// モデルの能力（対応するAPI）
///
/// モデルが対応する API エンドポイントを表す。
/// 1つのモデルが複数の能力を持つ場合がある（例: GPT-4o は TextGeneration + TextToSpeech）
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
    /// 埋め込み生成 (/v1/embeddings)
    Embedding,
}

impl ModelCapability {
    /// ModelType から推定されるデフォルトの capabilities を返す
    pub fn from_model_type(model_type: ModelType) -> Vec<Self> {
        match model_type {
            ModelType::Llm => vec![Self::TextGeneration],
            ModelType::Embedding => vec![Self::Embedding],
            ModelType::SpeechToText => vec![Self::SpeechToText],
            ModelType::TextToSpeech => vec![Self::TextToSpeech],
            ModelType::ImageGeneration => vec![Self::ImageGeneration],
        }
    }
}

/// モデルの能力（Azure OpenAI 形式）
///
/// Azure OpenAI API 互換の boolean object 形式で capabilities を表現。
/// `/v1/models` レスポンスで使用。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// チャット補完対応 (/v1/chat/completions)
    pub chat_completion: bool,
    /// テキスト補完対応 (/v1/completions)
    pub completion: bool,
    /// 埋め込み生成対応 (/v1/embeddings)
    pub embeddings: bool,
    /// ファインチューニング対応（未実装）
    pub fine_tune: bool,
    /// 推論対応（常に true）
    pub inference: bool,
    /// 音声合成対応 (/v1/audio/speech)
    pub text_to_speech: bool,
    /// 音声認識対応 (/v1/audio/transcriptions)
    pub speech_to_text: bool,
    /// 画像生成対応 (/v1/images/generations)
    pub image_generation: bool,
}

impl From<&[ModelCapability]> for ModelCapabilities {
    fn from(caps: &[ModelCapability]) -> Self {
        ModelCapabilities {
            chat_completion: caps.contains(&ModelCapability::TextGeneration),
            completion: caps.contains(&ModelCapability::TextGeneration),
            embeddings: caps.contains(&ModelCapability::Embedding),
            inference: true, // 全モデル対応
            text_to_speech: caps.contains(&ModelCapability::TextToSpeech),
            speech_to_text: caps.contains(&ModelCapability::SpeechToText),
            image_generation: caps.contains(&ModelCapability::ImageGeneration),
            fine_tune: false, // 未対応
        }
    }
}

impl From<Vec<ModelCapability>> for ModelCapabilities {
    fn from(caps: Vec<ModelCapability>) -> Self {
        ModelCapabilities::from(caps.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_serialization() {
        assert_eq!(serde_json::to_string(&ModelType::Llm).unwrap(), "\"llm\"");
        assert_eq!(
            serde_json::to_string(&ModelType::Embedding).unwrap(),
            "\"embedding\""
        );
        assert_eq!(
            serde_json::to_string(&ModelType::SpeechToText).unwrap(),
            "\"speech_to_text\""
        );
        assert_eq!(
            serde_json::to_string(&ModelType::TextToSpeech).unwrap(),
            "\"text_to_speech\""
        );
        assert_eq!(
            serde_json::to_string(&ModelType::ImageGeneration).unwrap(),
            "\"image_generation\""
        );
    }

    #[test]
    fn test_model_type_default() {
        let default_type: ModelType = Default::default();
        assert_eq!(default_type, ModelType::Llm);
    }

    #[test]
    fn test_model_type_deserialization() {
        let speech_to_text: ModelType = serde_json::from_str("\"speech_to_text\"").unwrap();
        assert_eq!(speech_to_text, ModelType::SpeechToText);

        let text_to_speech: ModelType = serde_json::from_str("\"text_to_speech\"").unwrap();
        assert_eq!(text_to_speech, ModelType::TextToSpeech);

        let image_gen: ModelType = serde_json::from_str("\"image_generation\"").unwrap();
        assert_eq!(image_gen, ModelType::ImageGeneration);
    }

    #[test]
    fn test_runtime_type_serialization() {
        assert_eq!(
            serde_json::to_string(&RuntimeType::LlamaCpp).unwrap(),
            "\"llama_cpp\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::NemotronCpp).unwrap(),
            "\"nemotron_cpp\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::GptOssCpp).unwrap(),
            "\"gptoss_cpp\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::SafetensorsCpp).unwrap(),
            "\"safetensors_cpp\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::WhisperCpp).unwrap(),
            "\"whisper_cpp\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::OnnxRuntime).unwrap(),
            "\"onnx_runtime\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeType::StableDiffusion).unwrap(),
            "\"stable_diffusion\""
        );
    }

    #[test]
    fn test_runtime_type_default() {
        let default_runtime: RuntimeType = Default::default();
        assert_eq!(default_runtime, RuntimeType::LlamaCpp);
    }

    #[test]
    fn test_runtime_type_deserialization() {
        let llama: RuntimeType = serde_json::from_str("\"llama_cpp\"").unwrap();
        assert_eq!(llama, RuntimeType::LlamaCpp);

        let nemotron: RuntimeType = serde_json::from_str("\"nemotron_cpp\"").unwrap();
        assert_eq!(nemotron, RuntimeType::NemotronCpp);

        let gptoss: RuntimeType = serde_json::from_str("\"gptoss_cpp\"").unwrap();
        assert_eq!(gptoss, RuntimeType::GptOssCpp);

        let safetensors: RuntimeType = serde_json::from_str("\"safetensors_cpp\"").unwrap();
        assert_eq!(safetensors, RuntimeType::SafetensorsCpp);

        let whisper: RuntimeType = serde_json::from_str("\"whisper_cpp\"").unwrap();
        assert_eq!(whisper, RuntimeType::WhisperCpp);

        let onnx: RuntimeType = serde_json::from_str("\"onnx_runtime\"").unwrap();
        assert_eq!(onnx, RuntimeType::OnnxRuntime);

        let sd: RuntimeType = serde_json::from_str("\"stable_diffusion\"").unwrap();
        assert_eq!(sd, RuntimeType::StableDiffusion);
    }

    #[test]
    fn test_model_capability_serialization() {
        assert_eq!(
            serde_json::to_string(&ModelCapability::TextGeneration).unwrap(),
            "\"text_generation\""
        );
        assert_eq!(
            serde_json::to_string(&ModelCapability::TextToSpeech).unwrap(),
            "\"text_to_speech\""
        );
        assert_eq!(
            serde_json::to_string(&ModelCapability::SpeechToText).unwrap(),
            "\"speech_to_text\""
        );
        assert_eq!(
            serde_json::to_string(&ModelCapability::ImageGeneration).unwrap(),
            "\"image_generation\""
        );
        assert_eq!(
            serde_json::to_string(&ModelCapability::Embedding).unwrap(),
            "\"embedding\""
        );
    }

    #[test]
    fn test_model_capability_deserialization() {
        let text_gen: ModelCapability = serde_json::from_str("\"text_generation\"").unwrap();
        assert_eq!(text_gen, ModelCapability::TextGeneration);

        let embedding: ModelCapability = serde_json::from_str("\"embedding\"").unwrap();
        assert_eq!(embedding, ModelCapability::Embedding);
    }

    #[test]
    fn test_model_capability_from_model_type() {
        let llm_caps = ModelCapability::from_model_type(ModelType::Llm);
        assert_eq!(llm_caps, vec![ModelCapability::TextGeneration]);

        let embed_caps = ModelCapability::from_model_type(ModelType::Embedding);
        assert_eq!(embed_caps, vec![ModelCapability::Embedding]);

        let stt_caps = ModelCapability::from_model_type(ModelType::SpeechToText);
        assert_eq!(stt_caps, vec![ModelCapability::SpeechToText]);

        let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
        assert_eq!(tts_caps, vec![ModelCapability::TextToSpeech]);

        let img_caps = ModelCapability::from_model_type(ModelType::ImageGeneration);
        assert_eq!(img_caps, vec![ModelCapability::ImageGeneration]);
    }

    #[test]
    fn test_model_capabilities_from_vec() {
        let llm_caps = vec![ModelCapability::TextGeneration];
        let caps: ModelCapabilities = llm_caps.into();
        assert!(caps.chat_completion);
        assert!(caps.completion);
        assert!(caps.inference);
        assert!(!caps.embeddings);
        assert!(!caps.text_to_speech);
        assert!(!caps.speech_to_text);
        assert!(!caps.image_generation);
        assert!(!caps.fine_tune);
    }

    #[test]
    fn test_model_capabilities_serialization() {
        let caps = ModelCapabilities {
            chat_completion: true,
            completion: true,
            embeddings: false,
            fine_tune: false,
            inference: true,
            text_to_speech: false,
            speech_to_text: false,
            image_generation: false,
        };

        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("\"chat_completion\":true"));
        assert!(json.contains("\"inference\":true"));

        let deserialized: ModelCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, deserialized);
    }
}
