//! 通信プロトコル定義
//!
//! Node↔Coordinator間の通信メッセージ

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use crate::types::{AudioFormat, GpuDeviceInfo};

/// ノード登録リクエスト
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterRequest {
    /// マシン名
    pub machine_name: String,
    /// IPアドレス
    pub ip_address: IpAddr,
    /// ランタイムバージョン（llama.cpp）
    #[serde(rename = "runtime_version", alias = "runtime_version")]
    pub runtime_version: String,
    /// ランタイムポート番号（推論用）
    #[serde(rename = "runtime_port", alias = "runtime_port")]
    pub runtime_port: u16,
    /// GPU利用可能フラグ
    pub gpu_available: bool,
    /// GPUデバイス情報
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpu_devices: Vec<GpuDeviceInfo>,
    /// GPU個数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<u32>,
    /// GPUモデル名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model: Option<String>,
}

/// ノード登録レスポンス
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterResponse {
    /// ノードID
    pub node_id: Uuid,
    /// ステータス ("registered" または "updated")
    pub status: RegisterStatus,
    /// ノードAPIポート（OpenAI互換API）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_api_port: Option<u16>,
    /// 自動配布されたモデル名（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_distributed_model: Option<String>,
    /// ダウンロードタスクID（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_task_id: Option<Uuid>,
    /// エージェントトークン（認証用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_token: Option<String>,
}

/// 登録ステータス
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RegisterStatus {
    /// 新規登録
    Registered,
    /// 既存ノード更新
    Updated,
}

/// ヘルスチェックリクエスト
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthCheckRequest {
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
    /// GPU計算能力 (例: "8.9")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア (0-10000)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_capability_score: Option<u32>,
    /// 処理中リクエスト数
    pub active_requests: u32,
    /// 過去N件の平均レスポンスタイム (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_response_time_ms: Option<f32>,
    /// ノードがロード済みのモデル一覧
    #[serde(default)]
    pub loaded_models: Vec<String>,
    /// ノードがロード済みのEmbeddingモデル一覧
    #[serde(default)]
    pub loaded_embedding_models: Vec<String>,
    /// ノードがロード済みのASRモデル一覧 (音声認識)
    #[serde(default)]
    pub loaded_asr_models: Vec<String>,
    /// ノードがロード済みのTTSモデル一覧 (音声合成)
    #[serde(default)]
    pub loaded_tts_models: Vec<String>,
    /// サポートするランタイム一覧
    #[serde(default)]
    pub supported_runtimes: Vec<crate::types::RuntimeType>,
    /// モデル起動中フラグ
    #[serde(default)]
    pub initializing: bool,
    /// 起動済みモデル数/総数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_models: Option<(u8, u8)>,
}

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// ロール ("user", "assistant", "system")
    pub role: String,
    /// メッセージ内容
    pub content: String,
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
    pub agent_machine_name: String,
    /// ノードのIPアドレス
    pub agent_ip: IpAddr,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_request_serialization() {
        let request = RegisterRequest {
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 11434,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "NVIDIA RTX 4090".to_string(),
                count: 2,
                memory: None,
            }],
            gpu_count: Some(2),
            gpu_model: Some("NVIDIA RTX 4090".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: RegisterRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_register_status_serialization() {
        assert_eq!(
            serde_json::to_string(&RegisterStatus::Registered).unwrap(),
            "\"registered\""
        );
        assert_eq!(
            serde_json::to_string(&RegisterStatus::Updated).unwrap(),
            "\"updated\""
        );
    }

    #[test]
    fn test_health_check_request_serialization() {
        let request = HealthCheckRequest {
            node_id: Uuid::new_v4(),
            cpu_usage: 45.5,
            memory_usage: 60.2,
            gpu_usage: Some(33.0),
            gpu_memory_usage: Some(71.0),
            gpu_memory_total_mb: Some(8192),
            gpu_memory_used_mb: Some(5800),
            gpu_temperature: Some(72.5),
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 3,
            average_response_time_ms: Some(123.4),
            loaded_models: vec!["gpt-oss-20b".to_string()],
            loaded_embedding_models: vec!["nomic-embed-text-v1.5".to_string()],
            loaded_asr_models: vec!["whisper-large-v3".to_string()],
            loaded_tts_models: vec!["vibevoice-v1".to_string()],
            supported_runtimes: vec![crate::types::RuntimeType::LlamaCpp],
            initializing: true,
            ready_models: Some((1, 2)),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: HealthCheckRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.cpu_usage, deserialized.cpu_usage);
        assert_eq!(request.memory_usage, deserialized.memory_usage);
        assert_eq!(request.gpu_usage, deserialized.gpu_usage);
        assert_eq!(request.gpu_memory_usage, deserialized.gpu_memory_usage);
        assert_eq!(request.active_requests, deserialized.active_requests);
        assert_eq!(
            request.average_response_time_ms,
            deserialized.average_response_time_ms
        );
        assert_eq!(request.loaded_models, deserialized.loaded_models);
        assert_eq!(
            request.loaded_embedding_models,
            deserialized.loaded_embedding_models
        );
        assert_eq!(request.loaded_asr_models, deserialized.loaded_asr_models);
        assert_eq!(request.loaded_tts_models, deserialized.loaded_tts_models);
    }

    #[test]
    fn test_health_check_request_with_gpu_capability() {
        let request = HealthCheckRequest {
            node_id: Uuid::new_v4(),
            cpu_usage: 50.0,
            memory_usage: 60.0,
            gpu_usage: Some(40.0),
            gpu_memory_usage: Some(50.0),
            gpu_memory_total_mb: Some(16384),
            gpu_memory_used_mb: Some(8192),
            gpu_temperature: Some(65.0),
            gpu_model_name: Some("NVIDIA GeForce RTX 4090".to_string()),
            gpu_compute_capability: Some("8.9".to_string()),
            gpu_capability_score: Some(9500),
            active_requests: 2,
            average_response_time_ms: Some(100.0),
            loaded_models: vec!["llama3:8b".to_string()],
            loaded_embedding_models: vec![],
            loaded_asr_models: vec![],
            loaded_tts_models: vec![],
            supported_runtimes: vec![],
            initializing: false,
            ready_models: Some((1, 1)),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: HealthCheckRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.gpu_model_name, deserialized.gpu_model_name);
        assert_eq!(
            request.gpu_compute_capability,
            deserialized.gpu_compute_capability
        );
        assert_eq!(
            request.gpu_capability_score,
            deserialized.gpu_capability_score
        );
    }

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
        use crate::types::AudioFormat;

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
}
