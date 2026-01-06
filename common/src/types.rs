//! 共通型定義
//!
//! Node, HealthMetrics, Request等のコアデータ型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
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

/// ノード
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    /// 一意識別子
    pub id: Uuid,
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
    /// 状態（オンライン/オフライン）
    pub status: NodeStatus,
    /// 登録日時
    pub registered_at: DateTime<Utc>,
    /// 最終ヘルスチェック時刻
    pub last_seen: DateTime<Utc>,
    /// 直近でオンライン状態に遷移した時刻（オンライン時のみ Some）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_since: Option<DateTime<Utc>>,
    /// カスタム表示名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
    /// タグ
    #[serde(default)]
    pub tags: Vec<String>,
    /// メモ
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// ロード済みモデル一覧
    #[serde(default)]
    pub loaded_models: Vec<String>,
    /// ロード済みEmbeddingモデル一覧
    #[serde(default)]
    pub loaded_embedding_models: Vec<String>,
    /// ロード済みASRモデル一覧 (音声認識)
    #[serde(default)]
    pub loaded_asr_models: Vec<String>,
    /// ロード済みTTSモデル一覧 (音声合成)
    #[serde(default)]
    pub loaded_tts_models: Vec<String>,
    /// サポートするランタイム一覧
    #[serde(default)]
    pub supported_runtimes: Vec<RuntimeType>,
    /// 搭載GPUの詳細
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpu_devices: Vec<GpuDeviceInfo>,
    /// GPU利用可能フラグ
    pub gpu_available: bool,
    /// GPU個数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_count: Option<u32>,
    /// GPUモデル名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model: Option<String>,
    /// GPUモデル名（詳細）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model_name: Option<String>,
    /// GPU計算能力 (例: "8.9")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_compute_capability: Option<String>,
    /// GPU能力スコア (0-10000)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_capability_score: Option<u32>,
    /// OpenAI互換APIポート（標準は runtime_port+1）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_api_port: Option<u16>,
    /// モデル起動中フラグ（全対応モデルが揃うまで true）
    #[serde(default)]
    pub initializing: bool,
    /// 起動済みモデル数/総数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_models: Option<(u8, u8)>,
    /// モデル同期状態
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_state: Option<SyncState>,
    /// モデル同期の進捗
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_progress: Option<SyncProgress>,
    /// 同期状態の最終更新時刻
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_updated_at: Option<DateTime<Utc>>,
    /// このノードで実行可能なモデルID一覧（ノードの/v1/modelsから取得）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executable_models: Vec<String>,
    /// 推論失敗等で一時的に除外されたモデルID一覧
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_models: Vec<String>,
}

/// ノード状態
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    /// 承認待ち
    Pending,
    /// オンライン（モデル同期完了）
    Online,
    /// 登録中（モデル同期中）
    Registering,
    /// オフライン
    Offline,
}

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

/// ノードメトリクス
///
/// ノードから定期的に送信される負荷情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    /// ノードID
    pub node_id: Uuid,
    /// CPU使用率（0.0〜100.0）
    pub cpu_usage: f64,
    /// メモリ使用率（0.0〜100.0）
    pub memory_usage: f64,
    /// アクティブリクエスト数
    pub active_requests: u32,
    /// 平均レスポンス時間（ミリ秒）
    pub avg_response_time_ms: Option<f64>,
    /// タイムスタンプ
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_serialization() {
        let node = Node {
            id: Uuid::new_v4(),
            machine_name: "test-machine".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            status: NodeStatus::Online,
            registered_at: Utc::now(),
            last_seen: Utc::now(),
            online_since: Some(Utc::now()),
            custom_name: Some("Custom".to_string()),
            tags: vec!["primary".to_string()],
            notes: Some("memo".to_string()),
            loaded_models: vec!["gpt-oss-20b".to_string()],
            loaded_embedding_models: vec!["nomic-embed-text-v1.5".to_string()],
            loaded_asr_models: vec!["whisper-large-v3".to_string()],
            loaded_tts_models: vec!["vibevoice-v1".to_string()],
            supported_runtimes: vec![RuntimeType::LlamaCpp, RuntimeType::WhisperCpp],
            gpu_devices: vec![GpuDeviceInfo {
                model: "NVIDIA RTX 4090".to_string(),
                count: 2,
                memory: None,
            }],
            gpu_available: true,
            gpu_count: Some(2),
            gpu_model: Some("NVIDIA RTX 4090".to_string()),
            gpu_model_name: Some("NVIDIA GeForce RTX 4090".to_string()),
            gpu_compute_capability: Some("8.9".to_string()),
            gpu_capability_score: Some(9850),
            node_api_port: Some(32769),
            initializing: false,
            ready_models: Some((1, 1)),
            sync_state: None,
            sync_progress: None,
            sync_updated_at: None,
            executable_models: vec!["gpt-oss-20b".to_string(), "nemotron-340b".to_string()],
            excluded_models: vec!["broken-model".to_string()],
        };

        let json = serde_json::to_string(&node).unwrap();
        let deserialized: Node = serde_json::from_str(&json).unwrap();

        assert_eq!(node, deserialized);
    }

    #[test]
    fn test_node_defaults() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "machine_name": "machine",
            "ip_address": "127.0.0.1",
            "runtime_version": "0.1.0",
            "runtime_port": 32768,
            "status": "online",
            "registered_at": "2025-10-31T00:00:00Z",
            "last_seen": "2025-10-31T00:00:00Z",
            "gpu_available": false
        }"#;

        let node: Node = serde_json::from_str(json).unwrap();
        assert!(node.custom_name.is_none());
        assert!(node.tags.is_empty());
        assert!(node.notes.is_none());
        assert!(node.loaded_models.is_empty());
        assert!(node.loaded_embedding_models.is_empty());
        assert!(node.gpu_devices.is_empty());
        assert!(!node.gpu_available);
        assert!(node.gpu_count.is_none());
        assert!(node.gpu_model.is_none());
        assert!(node.gpu_model_name.is_none());
        assert!(node.gpu_compute_capability.is_none());
        assert!(node.gpu_capability_score.is_none());
        assert!(node.online_since.is_none());
        assert!(node.sync_state.is_none());
        assert!(node.sync_progress.is_none());
        assert!(node.sync_updated_at.is_none());
        assert!(node.executable_models.is_empty());
        assert!(node.excluded_models.is_empty());
    }

    #[test]
    fn test_node_runtime_fields() {
        let json = r#"{
            "id": "00000000-0000-0000-0000-000000000000",
            "machine_name": "machine",
            "ip_address": "127.0.0.1",
            "runtime_version": "0.1.0",
            "runtime_port": 32768,
            "status": "online",
            "registered_at": "2025-10-31T00:00:00Z",
            "last_seen": "2025-10-31T00:00:00Z",
            "gpu_available": false
        }"#;

        let node: Node = serde_json::from_str(json).unwrap();
        assert_eq!(node.runtime_version, "0.1.0");
        assert_eq!(node.runtime_port, 32768);
    }

    #[test]
    fn test_node_status_serialization() {
        assert_eq!(
            serde_json::to_string(&NodeStatus::Online).unwrap(),
            "\"online\""
        );
        assert_eq!(
            serde_json::to_string(&NodeStatus::Registering).unwrap(),
            "\"registering\""
        );
        assert_eq!(
            serde_json::to_string(&NodeStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&NodeStatus::Offline).unwrap(),
            "\"offline\""
        );
    }

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

    #[test]
    fn test_node_metrics_serialization() {
        let node_id = Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap();
        let timestamp = DateTime::parse_from_rfc3339("2025-11-02T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let metrics = NodeMetrics {
            node_id,
            cpu_usage: 45.5,
            memory_usage: 60.2,
            active_requests: 3,
            avg_response_time_ms: Some(250.5),
            timestamp,
        };

        // JSON serialization
        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"node_id\":\"12345678-1234-1234-1234-123456789012\""));
        assert!(json.contains("\"cpu_usage\":45.5"));
        assert!(json.contains("\"memory_usage\":60.2"));
        assert!(json.contains("\"active_requests\":3"));
        assert!(json.contains("\"avg_response_time_ms\":250.5"));

        // JSON deserialization
        let deserialized: NodeMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_id, node_id);
        assert_eq!(deserialized.cpu_usage, 45.5);
        assert_eq!(deserialized.memory_usage, 60.2);
        assert_eq!(deserialized.active_requests, 3);
        assert_eq!(deserialized.avg_response_time_ms, Some(250.5));
        assert_eq!(deserialized.timestamp, timestamp);
    }

    #[test]
    fn test_node_metrics_deserialization_without_avg_response_time() {
        let json = r#"{
            "node_id": "12345678-1234-1234-1234-123456789012",
            "cpu_usage": 30.0,
            "memory_usage": 40.0,
            "active_requests": 2,
            "avg_response_time_ms": null,
            "timestamp": "2025-11-02T10:00:00Z"
        }"#;

        let metrics: NodeMetrics = serde_json::from_str(json).unwrap();
        assert_eq!(metrics.cpu_usage, 30.0);
        assert_eq!(metrics.memory_usage, 40.0);
        assert_eq!(metrics.active_requests, 2);
        assert_eq!(metrics.avg_response_time_ms, None);
    }

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
