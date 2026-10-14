//! 通信プロトコル定義
//!
//! OpenAI互換API用のリクエスト/レスポンス型を定義します。

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use crate::types::media::{AudioFormat, ImageQuality, ImageResponseFormat, ImageSize, ImageStyle};

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
    /// 処理したエンドポイントのID
    pub endpoint_id: Uuid,
    /// エンドポイント名
    pub endpoint_name: String,
    /// エンドポイントのIPアドレス
    pub endpoint_ip: IpAddr,
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
    /// /v1/messages エンドポイント (Anthropic Messages API)
    #[serde(rename = "anthropic_messages")]
    AnthropicMessages,
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
            RequestType::AnthropicMessages | RequestType::Chat => Some(Self::ChatCompletions),
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

impl RequestResponseRecord {
    /// エンドポイント特定済みのレコードを作成する。
    ///
    /// `status` から `RecordStatus` を自動判定する。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        endpoint_id: Uuid,
        endpoint_name: String,
        endpoint_ip: IpAddr,
        model: String,
        request_type: RequestType,
        request_body: serde_json::Value,
        status: StatusCode,
        duration: std::time::Duration,
        client_ip: Option<IpAddr>,
        api_key_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_type,
            model,
            endpoint_id,
            endpoint_name,
            endpoint_ip,
            client_ip,
            request_body,
            response_body: None,
            duration_ms: duration.as_millis() as u64,
            status: if status.is_success() {
                RecordStatus::Success
            } else {
                RecordStatus::Error {
                    message: format!("HTTP {}", status.as_u16()),
                }
            },
            completed_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            api_key_id,
        }
    }

    /// エンドポイント未特定のエラーレコードを作成する。
    pub fn error(
        model: String,
        request_type: RequestType,
        request_body: serde_json::Value,
        message: String,
        duration_ms: u64,
        client_ip: Option<IpAddr>,
        api_key_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            request_type,
            model,
            endpoint_id: Uuid::nil(),
            endpoint_name: "N/A".to_string(),
            endpoint_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            client_ip,
            request_body,
            response_body: None,
            duration_ms,
            status: RecordStatus::Error { message },
            completed_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            api_key_id,
        }
    }
}

#[cfg(test)]
mod tests;
