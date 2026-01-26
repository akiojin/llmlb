//! 共通型定義
//!
//! HealthMetrics, Request等のコアデータ型
//!
//! # SPEC-f8e3a1b7
//!
//! Node型は廃止されました。新しい実装では`Endpoint`型を使用してください。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// GPUデバイス情報
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuDeviceInfo {
    /// GPUモデル名
    pub model: String,
    /// 当該モデルの枚数
    pub count: u32,
    /// GPUメモリ容量（バイト単位、オプション）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
}

impl GpuDeviceInfo {
    /// GPU情報として有効か検証する
    pub fn is_valid(&self) -> bool {
        self.count > 0 && !self.model.trim().is_empty()
    }
}

// SPEC-f8e3a1b7: Node型とNodeStatus型は削除されました
// 新しい実装では Endpoint 型と EndpointStatus 型を使用してください

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
/// 1つのモデルが複数の能力を持つ場合がある（例: GPT-4o は TextGeneration + Vision + TextToSpeech）
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
    /// 画像理解対応 (/v1/chat/completions with images)
    #[serde(default)]
    pub image_understanding: bool,
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
            image_understanding: caps.contains(&ModelCapability::Vision),
            fine_tune: false, // 未対応
        }
    }
}

impl From<Vec<ModelCapability>> for ModelCapabilities {
    fn from(caps: Vec<ModelCapability>) -> Self {
        ModelCapabilities::from(caps.as_slice())
    }
}

/// 音声フォーマット
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV (PCM)
    Wav,
    /// MP3
    #[default]
    Mp3,
    /// FLAC (ロスレス)
    Flac,
    /// Ogg Vorbis
    Ogg,
    /// Opus
    Opus,
}

/// 画像MIMEタイプ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageContentType {
    /// image/jpeg
    #[serde(rename = "image/jpeg")]
    Jpeg,
    /// image/png
    #[serde(rename = "image/png")]
    Png,
    /// image/gif
    #[serde(rename = "image/gif")]
    Gif,
    /// image/webp
    #[serde(rename = "image/webp")]
    Webp,
}

impl ImageContentType {
    /// MIME文字列を返す
    pub fn as_mime(&self) -> &'static str {
        match self {
            ImageContentType::Jpeg => "image/jpeg",
            ImageContentType::Png => "image/png",
            ImageContentType::Gif => "image/gif",
            ImageContentType::Webp => "image/webp",
        }
    }
}

/// 画像データ（URLまたはBase64）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageContent {
    /// URL参照
    Url {
        /// 画像URL
        url: String,
        /// MIMEタイプ
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<ImageContentType>,
        /// サイズ（バイト）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_bytes: Option<u64>,
    },
    /// Base64エンコード
    Base64 {
        /// Base64文字列
        data: String,
        /// MIMEタイプ
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<ImageContentType>,
        /// サイズ（バイト）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size_bytes: Option<u64>,
    },
}

/// Vision対応能力
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisionCapability {
    /// 対応画像形式
    pub supported_formats: Vec<ImageContentType>,
    /// 画像サイズ上限（バイト）
    pub max_image_size_bytes: u64,
    /// 画像枚数上限（1リクエストあたり）
    pub max_image_count: u8,
}

impl Default for VisionCapability {
    fn default() -> Self {
        Self {
            supported_formats: vec![
                ImageContentType::Jpeg,
                ImageContentType::Png,
                ImageContentType::Gif,
                ImageContentType::Webp,
            ],
            max_image_size_bytes: 10 * 1024 * 1024,
            max_image_count: 10,
        }
    }
}

/// 画像サイズ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ImageSize {
    /// 256x256
    #[serde(rename = "256x256")]
    Size256,
    /// 512x512
    #[serde(rename = "512x512")]
    Size512,
    /// 1024x1024 (デフォルト)
    #[default]
    #[serde(rename = "1024x1024")]
    Size1024,
    /// 1792x1024 (横長)
    #[serde(rename = "1792x1024")]
    Size1792x1024,
    /// 1024x1792 (縦長)
    #[serde(rename = "1024x1792")]
    Size1024x1792,
}

/// 画像品質
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    /// 標準品質 (デフォルト)
    #[default]
    Standard,
    /// 高品質
    Hd,
}

/// 画像スタイル
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    /// 鮮やかなスタイル (デフォルト)
    #[default]
    Vivid,
    /// 自然なスタイル
    Natural,
}

/// 画像レスポンス形式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageResponseFormat {
    /// URL形式 (デフォルト)
    #[default]
    Url,
    /// Base64エンコード形式
    #[serde(rename = "b64_json")]
    B64Json,
}

/// ヘルスメトリクス
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthMetrics {
    /// ノードID
    pub node_id: Uuid,
    /// CPU使用率 (0.0-100.0)
    pub cpu_usage: f32,
    /// メモリ使用率 (0.0-100.0)
    pub memory_usage: f32,
    /// GPU使用率 (0.0-100.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_usage: Option<f32>,
    /// GPUメモリ使用率 (0.0-100.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_usage: Option<f32>,
    /// GPUメモリ総容量 (MB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_total_mb: Option<u64>,
    /// GPU使用メモリ (MB)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_used_mb: Option<u64>,
    /// GPU温度 (℃)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_temperature: Option<f32>,
    /// GPUモデル名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model_name: Option<String>,
    /// GPU計算能力
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_capability_score: Option<u32>,
    /// 処理中リクエスト数
    pub active_requests: u32,
    /// 累積リクエスト数
    pub total_requests: u64,
    /// 直近の平均レスポンスタイム (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_response_time_ms: Option<f32>,
    /// タイムスタンプ
    pub timestamp: DateTime<Utc>,
}

/// リクエスト
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    /// リクエストID
    pub id: Uuid,
    /// 振り分け先ノードID
    pub node_id: Uuid,
    /// エンドポイント ("/v1/chat/completions" など)
    pub endpoint: String,
    /// ステータス
    pub status: RequestStatus,
    /// 処理時間（ミリ秒）
    pub duration_ms: Option<u64>,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 完了日時
    pub completed_at: Option<DateTime<Utc>>,
}

/// リクエストステータス
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    /// 保留中
    Pending,
    /// 処理中
    Processing,
    /// 完了
    Completed,
    /// 失敗
    Failed,
}

// SPEC-f8e3a1b7: NodeMetrics型は削除されました
// エンドポイントベースのメトリクス収集を使用してください

#[cfg(test)]
mod tests {
    use super::*;

    // SPEC-f8e3a1b7: Node/NodeStatus/NodeMetrics関連のテストは削除されました
    // Endpoint型のテストは types/endpoint.rs に移動しました

    #[test]
    fn test_gpu_device_info_validation() {
        let valid = GpuDeviceInfo {
            model: "NVIDIA RTX 4090".to_string(),
            count: 2,
            memory: None,
        };
        assert!(valid.is_valid());

        let zero_count = GpuDeviceInfo {
            model: "AMD".to_string(),
            count: 0,
            memory: None,
        };
        assert!(!zero_count.is_valid());

        let empty_model = GpuDeviceInfo {
            model: " ".to_string(),
            count: 1,
            memory: None,
        };
        assert!(!empty_model.is_valid());
    }

    #[test]
    fn test_request_status_serialization() {
        assert_eq!(
            serde_json::to_string(&RequestStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&RequestStatus::Processing).unwrap(),
            "\"processing\""
        );
        assert_eq!(
            serde_json::to_string(&RequestStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&RequestStatus::Failed).unwrap(),
            "\"failed\""
        );
    }

    // SPEC-f8e3a1b7: test_node_metrics_* テストは削除されました

    #[test]
    fn test_model_type_serialization() {
        assert_eq!(serde_json::to_string(&ModelType::Llm).unwrap(), "\"llm\"");
        assert_eq!(
            serde_json::to_string(&ModelType::Embedding).unwrap(),
            "\"embedding\""
        );
        // 音声モデルタイプのシリアライズテスト
        assert_eq!(
            serde_json::to_string(&ModelType::SpeechToText).unwrap(),
            "\"speech_to_text\""
        );
        assert_eq!(
            serde_json::to_string(&ModelType::TextToSpeech).unwrap(),
            "\"text_to_speech\""
        );
    }

    #[test]
    fn test_model_type_default() {
        let default_type: ModelType = Default::default();
        assert_eq!(default_type, ModelType::Llm);
    }

    #[test]
    fn test_model_type_deserialization() {
        // 音声モデルタイプのデシリアライズテスト
        let speech_to_text: ModelType = serde_json::from_str("\"speech_to_text\"").unwrap();
        assert_eq!(speech_to_text, ModelType::SpeechToText);

        let text_to_speech: ModelType = serde_json::from_str("\"text_to_speech\"").unwrap();
        assert_eq!(text_to_speech, ModelType::TextToSpeech);
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
    }

    #[test]
    fn test_audio_format_serialization() {
        assert_eq!(serde_json::to_string(&AudioFormat::Wav).unwrap(), "\"wav\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Mp3).unwrap(), "\"mp3\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Flac).unwrap(),
            "\"flac\""
        );
        assert_eq!(serde_json::to_string(&AudioFormat::Ogg).unwrap(), "\"ogg\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Opus).unwrap(),
            "\"opus\""
        );
    }

    #[test]
    fn test_audio_format_default() {
        let default_format: AudioFormat = Default::default();
        assert_eq!(default_format, AudioFormat::Mp3);
    }

    #[test]
    fn test_audio_format_deserialization() {
        let wav: AudioFormat = serde_json::from_str("\"wav\"").unwrap();
        assert_eq!(wav, AudioFormat::Wav);

        let mp3: AudioFormat = serde_json::from_str("\"mp3\"").unwrap();
        assert_eq!(mp3, AudioFormat::Mp3);

        let flac: AudioFormat = serde_json::from_str("\"flac\"").unwrap();
        assert_eq!(flac, AudioFormat::Flac);
    }

    #[test]
    fn test_image_model_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ModelType::ImageGeneration).unwrap(),
            "\"image_generation\""
        );
    }

    #[test]
    fn test_image_model_type_deserialization() {
        let image_gen: ModelType = serde_json::from_str("\"image_generation\"").unwrap();
        assert_eq!(image_gen, ModelType::ImageGeneration);
    }

    #[test]
    fn test_stable_diffusion_runtime_serialization() {
        assert_eq!(
            serde_json::to_string(&RuntimeType::StableDiffusion).unwrap(),
            "\"stable_diffusion\""
        );
    }

    #[test]
    fn test_stable_diffusion_runtime_deserialization() {
        let sd: RuntimeType = serde_json::from_str("\"stable_diffusion\"").unwrap();
        assert_eq!(sd, RuntimeType::StableDiffusion);
    }

    #[test]
    fn test_image_size_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageSize::Size256).unwrap(),
            "\"256x256\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size512).unwrap(),
            "\"512x512\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1024).unwrap(),
            "\"1024x1024\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1792x1024).unwrap(),
            "\"1792x1024\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1024x1792).unwrap(),
            "\"1024x1792\""
        );
    }

    #[test]
    fn test_image_size_default() {
        let default_size: ImageSize = Default::default();
        assert_eq!(default_size, ImageSize::Size1024);
    }

    #[test]
    fn test_image_quality_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageQuality::Standard).unwrap(),
            "\"standard\""
        );
        assert_eq!(serde_json::to_string(&ImageQuality::Hd).unwrap(), "\"hd\"");
    }

    #[test]
    fn test_image_quality_default() {
        let default_quality: ImageQuality = Default::default();
        assert_eq!(default_quality, ImageQuality::Standard);
    }

    #[test]
    fn test_image_style_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageStyle::Vivid).unwrap(),
            "\"vivid\""
        );
        assert_eq!(
            serde_json::to_string(&ImageStyle::Natural).unwrap(),
            "\"natural\""
        );
    }

    #[test]
    fn test_image_style_default() {
        let default_style: ImageStyle = Default::default();
        assert_eq!(default_style, ImageStyle::Vivid);
    }

    #[test]
    fn test_image_response_format_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageResponseFormat::Url).unwrap(),
            "\"url\""
        );
        assert_eq!(
            serde_json::to_string(&ImageResponseFormat::B64Json).unwrap(),
            "\"b64_json\""
        );
    }

    #[test]
    fn test_image_response_format_default() {
        let default_format: ImageResponseFormat = Default::default();
        assert_eq!(default_format, ImageResponseFormat::Url);
    }

    // T002: ModelCapability serialization/deserialization tests
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
            serde_json::to_string(&ModelCapability::Vision).unwrap(),
            "\"vision\""
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

        let tts: ModelCapability = serde_json::from_str("\"text_to_speech\"").unwrap();
        assert_eq!(tts, ModelCapability::TextToSpeech);

        let stt: ModelCapability = serde_json::from_str("\"speech_to_text\"").unwrap();
        assert_eq!(stt, ModelCapability::SpeechToText);

        let image_gen: ModelCapability = serde_json::from_str("\"image_generation\"").unwrap();
        assert_eq!(image_gen, ModelCapability::ImageGeneration);

        let vision: ModelCapability = serde_json::from_str("\"vision\"").unwrap();
        assert_eq!(vision, ModelCapability::Vision);

        let embedding: ModelCapability = serde_json::from_str("\"embedding\"").unwrap();
        assert_eq!(embedding, ModelCapability::Embedding);
    }

    // T003: ModelCapability::from_model_type test
    #[test]
    fn test_model_capability_from_model_type() {
        // LLM → TextGeneration
        let llm_caps = ModelCapability::from_model_type(ModelType::Llm);
        assert_eq!(llm_caps, vec![ModelCapability::TextGeneration]);

        // Embedding → Embedding
        let embed_caps = ModelCapability::from_model_type(ModelType::Embedding);
        assert_eq!(embed_caps, vec![ModelCapability::Embedding]);

        // SpeechToText → SpeechToText
        let stt_caps = ModelCapability::from_model_type(ModelType::SpeechToText);
        assert_eq!(stt_caps, vec![ModelCapability::SpeechToText]);

        // TextToSpeech → TextToSpeech
        let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
        assert_eq!(tts_caps, vec![ModelCapability::TextToSpeech]);

        // ImageGeneration → ImageGeneration
        let img_caps = ModelCapability::from_model_type(ModelType::ImageGeneration);
        assert_eq!(img_caps, vec![ModelCapability::ImageGeneration]);
    }

    // ModelCapabilities (Azure形式) テスト
    #[test]
    fn test_model_capabilities_from_vec() {
        // LLM capabilities
        let llm_caps = vec![ModelCapability::TextGeneration];
        let caps: ModelCapabilities = llm_caps.into();
        assert!(caps.chat_completion);
        assert!(caps.completion);
        assert!(caps.inference);
        assert!(!caps.embeddings);
        assert!(!caps.text_to_speech);
        assert!(!caps.speech_to_text);
        assert!(!caps.image_generation);
        assert!(!caps.image_understanding);
        assert!(!caps.fine_tune);

        // Embedding capabilities
        let embed_caps = vec![ModelCapability::Embedding];
        let caps: ModelCapabilities = embed_caps.into();
        assert!(!caps.chat_completion);
        assert!(!caps.completion);
        assert!(caps.embeddings);
        assert!(caps.inference);
        assert!(!caps.image_understanding);

        // TTS capabilities
        let tts_caps = vec![ModelCapability::TextToSpeech];
        let caps: ModelCapabilities = tts_caps.into();
        assert!(caps.text_to_speech);
        assert!(!caps.speech_to_text);
        assert!(caps.inference);
        assert!(!caps.image_understanding);

        // ASR capabilities
        let stt_caps = vec![ModelCapability::SpeechToText];
        let caps: ModelCapabilities = stt_caps.into();
        assert!(caps.speech_to_text);
        assert!(!caps.text_to_speech);
        assert!(caps.inference);
        assert!(!caps.image_understanding);

        // Image generation capabilities
        let img_caps = vec![ModelCapability::ImageGeneration];
        let caps: ModelCapabilities = img_caps.into();
        assert!(caps.image_generation);
        assert!(caps.inference);
        assert!(!caps.image_understanding);

        // Vision capabilities
        let vision_caps = vec![ModelCapability::Vision];
        let caps: ModelCapabilities = vision_caps.into();
        assert!(caps.image_understanding);
        assert!(caps.inference);
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
            image_understanding: false,
        };

        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("\"chat_completion\":true"));
        assert!(json.contains("\"completion\":true"));
        assert!(json.contains("\"embeddings\":false"));
        assert!(json.contains("\"inference\":true"));
        assert!(json.contains("\"image_understanding\":false"));

        // Deserialization
        let deserialized: ModelCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, deserialized);
    }
}
