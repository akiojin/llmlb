//! 通信プロトコル定義
//!
//! OpenAI互換API用のリクエスト/レスポンス型を定義します。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use super::types::{AudioFormat, ImageQuality, ImageResponseFormat, ImageSize, ImageStyle};

/// LLM runtimeチャットリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// モデル名
    pub model: String,
    /// メッセージ配列
    pub messages: Vec<ChatMessage>,
    /// ストリーミング有効化
    #[serde(default)]
    pub stream: bool,
}

/// チャットメッセージ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    /// ロール ("user", "assistant", "system")
    pub role: String,
    /// メッセージ内容
    pub content: String,
}

/// Chat Completionsリクエスト
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCompletionRequest {
    /// モデル名
    pub model: String,
    /// メッセージ配列
    pub messages: Vec<ChatMessage>,
    /// ストリーミング有効化
    #[serde(default)]
    pub stream: bool,
    /// 最大トークン数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Generateリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// モデル名
    pub model: String,
    /// プロンプト
    pub prompt: String,
    /// ストリーミング有効化
    #[serde(default)]
    pub stream: bool,
}

/// リクエスト/レスポンスレコード
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestResponseRecord {
    /// レコードの一意識別子
    pub id: Uuid,
    /// リクエスト受信時刻
    pub timestamp: DateTime<Utc>,
    /// リクエストタイプ（Chat または Generate）
    pub request_type: RequestType,
    /// 使用されたモデル名
    pub model: String,
    /// 処理したノードのID
    pub node_id: Uuid,
    /// ノードのマシン名
    pub node_machine_name: String,
    /// ノードのIPアドレス
    pub node_ip: IpAddr,
    /// リクエスト元クライアントのIPアドレス
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<IpAddr>,
    /// リクエスト本文（JSON形式）
    pub request_body: serde_json::Value,
    /// レスポンス本文（JSON形式、エラー時はNone）
    pub response_body: Option<serde_json::Value>,
    /// 処理時間（ミリ秒）
    pub duration_ms: u64,
    /// レコードのステータス（成功 or エラー）
    pub status: RecordStatus,
    /// レスポンス完了時刻
    pub completed_at: DateTime<Utc>,
    /// 入力トークン数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    /// 出力トークン数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    /// 総トークン数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
    /// APIキーID（api_keysテーブル参照）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<Uuid>,
}

/// リクエストタイプ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestType {
    /// /v1/chat/completions エンドポイント
    Chat,
    /// /v1/completions エンドポイント
    Generate,
    /// /v1/embeddings エンドポイント
    Embeddings,
    /// /v1/audio/transcriptions エンドポイント (ASR)
    Transcription,
    /// /v1/audio/speech エンドポイント (TTS)
    Speech,
    /// /v1/images/generations エンドポイント
    ImageGeneration,
    /// /v1/images/edits エンドポイント
    ImageEdit,
    /// /v1/images/variations エンドポイント
    ImageVariation,
}

/// TPS計測対象のAPI種別。
///
/// 比較可能性を担保するため、TPSは API 種別ごとに分離して集計する。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TpsApiKind {
    /// /v1/chat/completions
    ChatCompletions,
    /// /v1/completions
    Completions,
    /// /v1/responses
    Responses,
}

impl TpsApiKind {
    /// RequestType から TPS API 種別を解決する。
    ///
    /// テキスト生成系以外（embeddings/audio/images）は TPS対象外として None を返す。
    pub fn from_request_type(request_type: RequestType) -> Option<Self> {
        match request_type {
            RequestType::Chat => Some(Self::ChatCompletions),
            RequestType::Generate => Some(Self::Completions),
            RequestType::Embeddings
            | RequestType::Transcription
            | RequestType::Speech
            | RequestType::ImageGeneration
            | RequestType::ImageEdit
            | RequestType::ImageVariation => None,
        }
    }
}

/// TPSデータの取得元。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TpsSource {
    /// 本番リクエスト由来
    Production,
    /// ベンチマーク実行由来
    Benchmark,
}

/// レコードステータス
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RecordStatus {
    /// 正常に処理完了
    Success,
    /// エラー発生
    Error {
        /// エラーメッセージ
        message: String,
    },
}

/// 音声認識レスポンスフォーマット
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionResponseFormat {
    /// JSON形式
    #[default]
    Json,
    /// テキスト形式
    Text,
    /// SRT字幕形式
    Srt,
    /// VTT字幕形式
    Vtt,
    /// 詳細JSON形式（タイムスタンプ付き）
    VerboseJson,
}

/// 音声認識リクエスト (ASR)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionRequest {
    /// モデル名 (例: "whisper-large-v3")
    pub model: String,
    /// 音声の言語 (ISO-639-1形式、例: "ja", "en")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// レスポンスフォーマット
    #[serde(default)]
    pub response_format: TranscriptionResponseFormat,
    /// サンプリング温度 (0.0-1.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// タイムスタンプの粒度 (segment, word)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<String>>,
}

/// 音声認識レスポンス (ASR)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionResponse {
    /// 認識されたテキスト
    pub text: String,
    /// 検出された言語
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// 音声の長さ（秒）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// セグメント情報（verbose_json時）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,
}

/// 音声認識セグメント
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionSegment {
    /// セグメントID
    pub id: u32,
    /// 開始時間（秒）
    pub start: f64,
    /// 終了時間（秒）
    pub end: f64,
    /// セグメントテキスト
    pub text: String,
}

/// 音声合成リクエスト (TTS)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpeechRequest {
    /// モデル名 (例: "vibevoice-v1", "tts-1")
    pub model: String,
    /// 読み上げテキスト
    pub input: String,
    /// ボイス名 (例: "nova", "alloy", "echo")
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 出力フォーマット
    #[serde(default)]
    pub response_format: AudioFormat,
    /// 再生速度 (0.25-4.0、デフォルト1.0)
    #[serde(default = "default_speed")]
    pub speed: f64,
}

fn default_voice() -> String {
    "nova".to_string()
}

fn default_speed() -> f64 {
    1.0
}

/// 画像生成リクエスト (Text-to-Image)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationRequest {
    /// モデル名 (例: "stable-diffusion-xl", "dall-e-3")
    pub model: String,
    /// 生成プロンプト
    pub prompt: String,
    /// 生成画像数 (1-10、デフォルト1)
    #[serde(default = "default_image_n")]
    pub n: u8,
    /// 出力サイズ
    #[serde(default)]
    pub size: ImageSize,
    /// 品質設定
    #[serde(default)]
    pub quality: ImageQuality,
    /// スタイル
    #[serde(default)]
    pub style: ImageStyle,
    /// レスポンスフォーマット
    #[serde(default)]
    pub response_format: ImageResponseFormat,
    /// ネガティブプロンプト（SD拡張）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// シード値（再現性用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    /// 生成ステップ数（SD拡張、デフォルト: 20）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
}

fn default_image_n() -> u8 {
    1
}

/// 画像編集リクエスト (Inpainting)
///
/// multipart/form-dataとして送信されるため、画像データは別途処理
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageEditRequest {
    /// モデル名
    pub model: String,
    /// 編集プロンプト
    pub prompt: String,
    /// 生成画像数 (1-10、デフォルト1)
    #[serde(default = "default_image_n")]
    pub n: u8,
    /// 出力サイズ
    #[serde(default)]
    pub size: ImageSize,
    /// レスポンスフォーマット
    #[serde(default)]
    pub response_format: ImageResponseFormat,
}

/// 画像バリエーションリクエスト
///
/// multipart/form-dataとして送信されるため、画像データは別途処理
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageVariationRequest {
    /// モデル名
    pub model: String,
    /// 生成画像数 (1-10、デフォルト1)
    #[serde(default = "default_image_n")]
    pub n: u8,
    /// 出力サイズ
    #[serde(default)]
    pub size: ImageSize,
    /// レスポンスフォーマット
    #[serde(default)]
    pub response_format: ImageResponseFormat,
}

/// 画像レスポンス (generations/edits/variations共通)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageResponse {
    /// 生成時刻 (Unix timestamp)
    pub created: i64,
    /// 生成された画像データ配列
    pub data: Vec<ImageData>,
}

/// 画像データ
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ImageData {
    /// URL形式
    Url {
        /// 画像URL
        url: String,
        /// 改訂されたプロンプト（DALL-E 3等）
        #[serde(skip_serializing_if = "Option::is_none")]
        revised_prompt: Option<String>,
    },
    /// Base64形式
    Base64 {
        /// Base64エンコードされた画像データ
        b64_json: String,
        /// 改訂されたプロンプト
        #[serde(skip_serializing_if = "Option::is_none")]
        revised_prompt: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_default_stream_false() {
        let json = r#"{"model":"llama2","messages":[{"role":"user","content":"Hello"}]}"#;
        let request: ChatRequest = serde_json::from_str(json).unwrap();

        assert!(!request.stream);
    }

    #[test]
    fn test_request_type_serialization() {
        assert_eq!(
            serde_json::to_string(&RequestType::Chat).unwrap(),
            "\"chat\""
        );
        assert_eq!(
            serde_json::to_string(&RequestType::Generate).unwrap(),
            "\"generate\""
        );
        assert_eq!(
            serde_json::to_string(&RequestType::Embeddings).unwrap(),
            "\"embeddings\""
        );
        // 音声リクエストタイプ
        assert_eq!(
            serde_json::to_string(&RequestType::Transcription).unwrap(),
            "\"transcription\""
        );
        assert_eq!(
            serde_json::to_string(&RequestType::Speech).unwrap(),
            "\"speech\""
        );
    }

    #[test]
    fn test_transcription_request_serialization() {
        let request = TranscriptionRequest {
            model: "whisper-large-v3".to_string(),
            language: Some("ja".to_string()),
            response_format: TranscriptionResponseFormat::Json,
            temperature: None,
            timestamp_granularities: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"whisper-large-v3\""));
        assert!(json.contains("\"language\":\"ja\""));
        assert!(json.contains("\"response_format\":\"json\""));

        let deserialized: TranscriptionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, "whisper-large-v3");
        assert_eq!(deserialized.language, Some("ja".to_string()));
    }

    #[test]
    fn test_transcription_response_format_serialization() {
        assert_eq!(
            serde_json::to_string(&TranscriptionResponseFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&TranscriptionResponseFormat::Text).unwrap(),
            "\"text\""
        );
        assert_eq!(
            serde_json::to_string(&TranscriptionResponseFormat::Srt).unwrap(),
            "\"srt\""
        );
        assert_eq!(
            serde_json::to_string(&TranscriptionResponseFormat::Vtt).unwrap(),
            "\"vtt\""
        );
        assert_eq!(
            serde_json::to_string(&TranscriptionResponseFormat::VerboseJson).unwrap(),
            "\"verbose_json\""
        );
    }

    #[test]
    fn test_transcription_response_serialization() {
        let response = TranscriptionResponse {
            text: "こんにちは".to_string(),
            language: Some("ja".to_string()),
            duration: Some(2.5),
            segments: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"text\":\"こんにちは\""));
        assert!(json.contains("\"language\":\"ja\""));

        let deserialized: TranscriptionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.text, "こんにちは");
    }

    #[test]
    fn test_speech_request_serialization() {
        use super::super::types::AudioFormat;

        let request = SpeechRequest {
            model: "vibevoice-v1".to_string(),
            input: "こんにちは".to_string(),
            voice: "nova".to_string(),
            response_format: AudioFormat::Mp3,
            speed: 1.0,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"vibevoice-v1\""));
        assert!(json.contains("\"input\":\"こんにちは\""));
        assert!(json.contains("\"voice\":\"nova\""));
        assert!(json.contains("\"response_format\":\"mp3\""));
        assert!(json.contains("\"speed\":1.0") || json.contains("\"speed\":1"));

        let deserialized: SpeechRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, "vibevoice-v1");
        assert_eq!(deserialized.input, "こんにちは");
        assert_eq!(deserialized.voice, "nova");
    }

    #[test]
    fn test_speech_request_defaults() {
        let json = r#"{"model":"tts-1","input":"Hello"}"#;
        let request: SpeechRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.model, "tts-1");
        assert_eq!(request.input, "Hello");
        assert_eq!(request.voice, "nova"); // default
        assert_eq!(request.speed, 1.0); // default
    }

    #[test]
    fn test_request_type_image_serialization() {
        assert_eq!(
            serde_json::to_string(&RequestType::ImageGeneration).unwrap(),
            "\"imagegeneration\""
        );
        assert_eq!(
            serde_json::to_string(&RequestType::ImageEdit).unwrap(),
            "\"imageedit\""
        );
        assert_eq!(
            serde_json::to_string(&RequestType::ImageVariation).unwrap(),
            "\"imagevariation\""
        );
    }

    #[test]
    fn test_image_generation_request_serialization() {
        use super::super::types::{ImageQuality, ImageResponseFormat, ImageSize, ImageStyle};

        let request = ImageGenerationRequest {
            model: "stable-diffusion-xl".to_string(),
            prompt: "A white cat sitting on a windowsill".to_string(),
            n: 2,
            size: ImageSize::Size1024,
            quality: ImageQuality::Standard,
            style: ImageStyle::Vivid,
            response_format: ImageResponseFormat::Url,
            negative_prompt: Some("blurry".to_string()),
            seed: Some(12345),
            steps: Some(2),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"stable-diffusion-xl\""));
        assert!(json.contains("\"prompt\":\"A white cat sitting on a windowsill\""));
        assert!(json.contains("\"n\":2"));
        assert!(json.contains("\"size\":\"1024x1024\""));
        assert!(json.contains("\"negative_prompt\":\"blurry\""));
        assert!(json.contains("\"steps\":2"));

        let deserialized: ImageGenerationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, "stable-diffusion-xl");
        assert_eq!(deserialized.n, 2);
    }

    #[test]
    fn test_image_generation_request_defaults() {
        let json = r#"{"model":"sd-xl","prompt":"A cat"}"#;
        let request: ImageGenerationRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.model, "sd-xl");
        assert_eq!(request.prompt, "A cat");
        assert_eq!(request.n, 1); // default
        assert_eq!(request.size, ImageSize::Size1024); // default
        assert_eq!(request.quality, ImageQuality::Standard); // default
        assert_eq!(request.style, ImageStyle::Vivid); // default
        assert_eq!(request.response_format, ImageResponseFormat::Url); // default
        assert_eq!(request.negative_prompt, None);
        assert_eq!(request.seed, None);
        assert_eq!(request.steps, None);
    }

    #[test]
    fn test_image_edit_request_serialization() {
        let request = ImageEditRequest {
            model: "stable-diffusion-xl".to_string(),
            prompt: "Add a hat".to_string(),
            n: 1,
            size: ImageSize::Size512,
            response_format: ImageResponseFormat::B64Json,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"stable-diffusion-xl\""));
        assert!(json.contains("\"prompt\":\"Add a hat\""));
        assert!(json.contains("\"size\":\"512x512\""));
        assert!(json.contains("\"response_format\":\"b64_json\""));

        let deserialized: ImageEditRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.prompt, "Add a hat");
    }

    #[test]
    fn test_image_variation_request_serialization() {
        let request = ImageVariationRequest {
            model: "stable-diffusion-xl".to_string(),
            n: 3,
            size: ImageSize::Size256,
            response_format: ImageResponseFormat::Url,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"stable-diffusion-xl\""));
        assert!(json.contains("\"n\":3"));
        assert!(json.contains("\"size\":\"256x256\""));

        let deserialized: ImageVariationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.n, 3);
    }

    #[test]
    fn test_image_response_serialization_url() {
        let response = ImageResponse {
            created: 1699000000,
            data: vec![ImageData::Url {
                url: "https://example.com/image.png".to_string(),
                revised_prompt: Some("A beautiful cat".to_string()),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"created\":1699000000"));
        assert!(json.contains("\"url\":\"https://example.com/image.png\""));
        assert!(json.contains("\"revised_prompt\":\"A beautiful cat\""));

        let deserialized: ImageResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.created, 1699000000);
        assert_eq!(deserialized.data.len(), 1);
    }

    #[test]
    fn test_image_response_serialization_base64() {
        let response = ImageResponse {
            created: 1699000000,
            data: vec![ImageData::Base64 {
                b64_json: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string(),
                revised_prompt: None,
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"created\":1699000000"));
        assert!(json.contains("\"b64_json\":"));

        let deserialized: ImageResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data.len(), 1);
    }

    #[test]
    fn test_request_response_record_token_fields_serialization() {
        // T-1: RequestResponseRecordのトークンフィールドシリアライズテスト
        let record = RequestResponseRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_type: RequestType::Chat,
            model: "gpt-3.5-turbo".to_string(),
            node_id: Uuid::new_v4(),
            node_machine_name: "test-node".to_string(),
            node_ip: "127.0.0.1".parse().unwrap(),
            client_ip: Some("192.168.1.1".parse().unwrap()),
            request_body: serde_json::json!({"messages": []}),
            response_body: Some(serde_json::json!({"choices": []})),
            duration_ms: 100,
            status: RecordStatus::Success,
            completed_at: Utc::now(),
            input_tokens: Some(150),
            output_tokens: Some(50),
            total_tokens: Some(200),
            api_key_id: None,
        };

        let json = serde_json::to_string(&record).unwrap();

        // トークンフィールドがシリアライズされていることを確認
        assert!(json.contains("\"input_tokens\":150"));
        assert!(json.contains("\"output_tokens\":50"));
        assert!(json.contains("\"total_tokens\":200"));

        // デシリアライズして値を確認
        let deserialized: RequestResponseRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.input_tokens, Some(150));
        assert_eq!(deserialized.output_tokens, Some(50));
        assert_eq!(deserialized.total_tokens, Some(200));
    }

    #[test]
    fn test_request_response_record_token_fields_none() {
        // トークンフィールドがNoneの場合のテスト
        let record = RequestResponseRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_type: RequestType::Chat,
            model: "gpt-3.5-turbo".to_string(),
            node_id: Uuid::new_v4(),
            node_machine_name: "test-node".to_string(),
            node_ip: "127.0.0.1".parse().unwrap(),
            client_ip: None,
            request_body: serde_json::json!({}),
            response_body: None,
            duration_ms: 50,
            status: RecordStatus::Success,
            completed_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            api_key_id: None,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: RequestResponseRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.input_tokens, None);
        assert_eq!(deserialized.output_tokens, None);
        assert_eq!(deserialized.total_tokens, None);
    }
}
