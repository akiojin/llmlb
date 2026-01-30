//! エンドポイント型定義
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// デバイスタイプ（SPEC-f8e3a1b7）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// CPU推論
    #[default]
    Cpu,
    /// GPU推論
    Gpu,
}

/// デバイス情報（SPEC-f8e3a1b7）
///
/// /api/system APIから取得したデバイス情報を格納
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceInfo {
    /// デバイスタイプ（CPU/GPU）
    pub device_type: DeviceType,
    /// GPUデバイス情報（GPU推論の場合のみ）
    #[serde(default)]
    pub gpu_devices: Vec<GpuDevice>,
}

/// GPU デバイス情報（SPEC-f8e3a1b7）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    /// デバイス名（例: "NVIDIA RTX 4090"）
    pub name: String,
    /// 総メモリ（バイト）
    pub total_memory_bytes: u64,
    /// 使用中メモリ（バイト）
    #[serde(default)]
    pub used_memory_bytes: u64,
}

/// モデルがサポートするAPI種別（SPEC-24157000）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupportedAPI {
    /// Chat Completions API（/v1/chat/completions）
    ChatCompletions,
    /// Responses API（/v1/responses）
    Responses,
    /// Embeddings API（/v1/embeddings）
    Embeddings,
}

impl SupportedAPI {
    /// SupportedAPIを文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Responses => "responses",
            Self::Embeddings => "embeddings",
        }
    }
}

impl std::fmt::Display for SupportedAPI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// エンドポイントの状態
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EndpointStatus {
    /// 初期状態（未確認）
    #[default]
    Pending,
    /// 稼働中
    Online,
    /// 停止中
    Offline,
    /// エラー状態
    Error,
}

impl EndpointStatus {
    /// EndpointStatusを文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Error => "error",
        }
    }
}

impl FromStr for EndpointStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pending" => Self::Pending,
            "online" => Self::Online,
            "offline" => Self::Offline,
            "error" => Self::Error,
            _ => Self::Pending,
        })
    }
}

impl std::fmt::Display for EndpointStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// エンドポイントタイプ（SPEC-66555000追加要件 2026-01-26）
///
/// エンドポイントの種別を表す列挙型。
/// 登録時に自動判別され、タイプに応じた機能制御に使用される。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EndpointType {
    /// 本プロジェクト独自の推論エンジン（xLLM）
    Xllm,
    /// Ollamaサーバー
    Ollama,
    /// vLLMサーバー
    Vllm,
    /// その他のOpenAI互換API
    OpenaiCompatible,
    /// 判別不能（オフライン時）
    #[default]
    Unknown,
}

impl EndpointType {
    /// EndpointTypeを文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Xllm => "xllm",
            Self::Ollama => "ollama",
            Self::Vllm => "vllm",
            Self::OpenaiCompatible => "openai_compatible",
            Self::Unknown => "unknown",
        }
    }

    /// モデルダウンロードをサポートするか
    pub fn supports_model_download(&self) -> bool {
        matches!(self, Self::Xllm)
    }

    /// モデルメタデータ取得をサポートするか
    pub fn supports_model_metadata(&self) -> bool {
        matches!(self, Self::Xllm | Self::Ollama)
    }
}

impl FromStr for EndpointType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "xllm" => Self::Xllm,
            "ollama" => Self::Ollama,
            "vllm" => Self::Vllm,
            "openai_compatible" => Self::OpenaiCompatible,
            _ => Self::Unknown,
        })
    }
}

impl std::fmt::Display for EndpointType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// ダウンロードタスクの状態（SPEC-66555000追加要件 2026-01-26）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// 待機中
    #[default]
    Pending,
    /// ダウンロード中
    Downloading,
    /// 完了
    Completed,
    /// 失敗
    Failed,
    /// キャンセル
    Cancelled,
}

impl DownloadStatus {
    /// DownloadStatusを文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Downloading => "downloading",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for DownloadStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pending" => Self::Pending,
            "downloading" => Self::Downloading,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        })
    }
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// エンドポイントの機能タイプ
///
/// NodeのRuntimeTypeに相当する機能分類（SPEC-66555000移行用）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EndpointCapability {
    /// チャット補完（LLM推論）
    ChatCompletion,
    /// 埋め込みベクトル生成
    Embeddings,
    /// 画像生成（StableDiffusion等）
    ImageGeneration,
    /// 音声認識（Whisper等）
    AudioTranscription,
    /// 音声合成（TTS）
    AudioSpeech,
}

impl EndpointCapability {
    /// 文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ChatCompletion => "chat_completion",
            Self::Embeddings => "embeddings",
            Self::ImageGeneration => "image_generation",
            Self::AudioTranscription => "audio_transcription",
            Self::AudioSpeech => "audio_speech",
        }
    }
}

impl std::fmt::Display for EndpointCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// エンドポイント
///
/// 推論サービスの接続先を表すエンティティ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// 一意識別子
    pub id: Uuid,
    /// 表示名（例: "本番Ollama", "開発xLLM1"）
    pub name: String,
    /// ベースURL（例: `http://192.168.1.100:11434`）
    pub base_url: String,
    /// APIキー（暗号化保存、シリアライズ時はスキップ）
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    /// 現在の状態
    pub status: EndpointStatus,
    /// エンドポイントタイプ（SPEC-66555000追加要件 2026-01-26）
    #[serde(default)]
    pub endpoint_type: EndpointType,
    /// ヘルスチェック間隔（秒）
    pub health_check_interval_secs: u32,
    /// 推論タイムアウト（秒）
    pub inference_timeout_secs: u32,
    /// ヘルスチェック時のレイテンシ（ミリ秒）
    pub latency_ms: Option<u32>,
    /// 最終確認時刻
    pub last_seen: Option<DateTime<Utc>>,
    /// 最後のエラーメッセージ
    pub last_error: Option<String>,
    /// 連続エラー回数
    pub error_count: u32,
    /// 登録日時
    pub registered_at: DateTime<Utc>,
    /// メモ
    pub notes: Option<String>,
    /// Responses API対応フラグ（SPEC-24157000）
    #[serde(default)]
    pub supports_responses_api: bool,
    /// エンドポイントの機能一覧（SPEC-66555000移行用）
    /// 画像生成、音声認識等の特殊機能をサポートするかを示す
    #[serde(default)]
    pub capabilities: Vec<EndpointCapability>,
    /// GPU情報（/api/healthから取得、Phase 1.4）
    #[serde(default)]
    pub gpu_device_count: Option<u32>,
    /// GPU総メモリ（バイト）
    #[serde(default)]
    pub gpu_total_memory_bytes: Option<u64>,
    /// GPU使用中メモリ（バイト）
    #[serde(default)]
    pub gpu_used_memory_bytes: Option<u64>,
    /// GPU能力スコア
    #[serde(default)]
    pub gpu_capability_score: Option<f32>,
    /// 現在のアクティブリクエスト数
    #[serde(default)]
    pub active_requests: Option<u32>,
    /// デバイス情報（/api/systemから取得、SPEC-f8e3a1b7）
    #[serde(default)]
    pub device_info: Option<DeviceInfo>,
    /// 推論レイテンシ（EMA、ミリ秒、SPEC-f8e3a1b7）
    /// ヘルスチェックのlatency_msとは別に、実際の推論時間を追跡
    #[serde(default)]
    pub inference_latency_ms: Option<f64>,
}

impl Endpoint {
    /// 新しいエンドポイントを作成
    pub fn new(name: String, base_url: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            base_url,
            api_key: None,
            status: EndpointStatus::Pending,
            endpoint_type: EndpointType::Unknown,
            health_check_interval_secs: 30,
            inference_timeout_secs: 120,
            latency_ms: None,
            last_seen: None,
            last_error: None,
            error_count: 0,
            registered_at: Utc::now(),
            notes: None,
            supports_responses_api: false,
            capabilities: vec![EndpointCapability::ChatCompletion], // デフォルトはチャット機能
            gpu_device_count: None,
            gpu_total_memory_bytes: None,
            gpu_used_memory_bytes: None,
            gpu_capability_score: None,
            active_requests: None,
            device_info: None,
            inference_latency_ms: None,
        }
    }

    /// 指定した機能をサポートしているか確認
    pub fn has_capability(&self, cap: EndpointCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// 推論レイテンシを更新（EMA α=0.2）（SPEC-f8e3a1b7）
    ///
    /// 新しい計測値を指数移動平均で反映する。
    /// 初回計測時はその値をそのまま設定。
    pub fn update_inference_latency(&mut self, new_latency_ms: f64) {
        const ALPHA: f64 = 0.2;
        self.inference_latency_ms = Some(match self.inference_latency_ms {
            Some(current) if current.is_finite() => {
                ALPHA * new_latency_ms + (1.0 - ALPHA) * current
            }
            _ => new_latency_ms,
        });
    }

    /// オフライン時にレイテンシを無限大にリセット（SPEC-f8e3a1b7）
    ///
    /// エンドポイントがオフラインになった場合、負荷分散で最低優先度になるよう
    /// レイテンシを無限大に設定する。
    pub fn reset_inference_latency(&mut self) {
        self.inference_latency_ms = Some(f64::INFINITY);
    }

    /// 推論レイテンシを取得（ソート用、未計測時は無限大）
    pub fn get_inference_latency_for_sort(&self) -> f64 {
        self.inference_latency_ms.unwrap_or(f64::INFINITY)
    }

    // SPEC-f8e3a1b7: to_legacy_node()は削除されました
    // Node型は完全に廃止され、Endpoint型に移行しました
}

/// エンドポイントで利用可能なモデル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointModel {
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// モデル識別子
    pub model_id: String,
    /// 能力（chat, embeddings等）
    pub capabilities: Option<Vec<String>>,
    /// 最大トークン数（SPEC-66555000追加要件 2026-01-26）
    pub max_tokens: Option<u32>,
    /// 最終確認時刻
    pub last_checked: Option<DateTime<Utc>>,
    /// サポートするAPI一覧（SPEC-24157000）
    #[serde(default = "EndpointModel::default_supported_apis")]
    pub supported_apis: Vec<SupportedAPI>,
}

impl EndpointModel {
    /// デフォルトのサポートAPI（Chat Completionsのみ）
    fn default_supported_apis() -> Vec<SupportedAPI> {
        vec![SupportedAPI::ChatCompletions]
    }
}

/// モデルダウンロードタスク（SPEC-66555000追加要件 2026-01-26）
///
/// xLLMエンドポイント専用のモデルダウンロード進捗管理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadTask {
    /// タスク識別子
    pub id: String,
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// モデル名（例: "llama-3.2-1b"）
    pub model: String,
    /// ダウンロード中のファイル名
    pub filename: Option<String>,
    /// ダウンロード状態
    pub status: DownloadStatus,
    /// 進捗率（0.0 〜 1.0）
    pub progress: f64,
    /// ダウンロード速度（Mbps）
    pub speed_mbps: Option<f64>,
    /// 残り時間（秒）
    pub eta_seconds: Option<u32>,
    /// エラーメッセージ（失敗時のみ）
    pub error_message: Option<String>,
    /// 開始時刻
    pub started_at: DateTime<Utc>,
    /// 完了時刻
    pub completed_at: Option<DateTime<Utc>>,
}

impl ModelDownloadTask {
    /// 新しいダウンロードタスクを作成
    pub fn new(endpoint_id: Uuid, model: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            endpoint_id,
            model,
            filename: None,
            status: DownloadStatus::Pending,
            progress: 0.0,
            speed_mbps: None,
            eta_seconds: None,
            error_message: None,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// ダウンロード完了かどうか
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
        )
    }
}

/// ヘルスチェック履歴
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealthCheck {
    /// 自動インクリメントID
    pub id: i64,
    /// エンドポイントID
    pub endpoint_id: Uuid,
    /// チェック実行時刻
    pub checked_at: DateTime<Utc>,
    /// 成功/失敗
    pub success: bool,
    /// レイテンシ（成功時のみ）
    pub latency_ms: Option<u32>,
    /// エラーメッセージ（失敗時のみ）
    pub error_message: Option<String>,
    /// チェック前の状態
    pub status_before: EndpointStatus,
    /// チェック後の状態
    pub status_after: EndpointStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_status_serialization() {
        assert_eq!(
            serde_json::to_string(&EndpointStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointStatus::Online).unwrap(),
            "\"online\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointStatus::Offline).unwrap(),
            "\"offline\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointStatus::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn test_endpoint_status_from_str() {
        assert_eq!(
            "pending".parse::<EndpointStatus>().unwrap(),
            EndpointStatus::Pending
        );
        assert_eq!(
            "online".parse::<EndpointStatus>().unwrap(),
            EndpointStatus::Online
        );
        assert_eq!(
            "offline".parse::<EndpointStatus>().unwrap(),
            EndpointStatus::Offline
        );
        assert_eq!(
            "error".parse::<EndpointStatus>().unwrap(),
            EndpointStatus::Error
        );
        assert_eq!(
            "unknown".parse::<EndpointStatus>().unwrap(),
            EndpointStatus::Pending
        );
    }

    #[test]
    fn test_endpoint_new() {
        let endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        assert_eq!(endpoint.name, "Test");
        assert_eq!(endpoint.base_url, "http://localhost:8080");
        assert_eq!(endpoint.status, EndpointStatus::Pending);
        assert_eq!(endpoint.health_check_interval_secs, 30);
        assert_eq!(endpoint.inference_timeout_secs, 120);
        assert_eq!(endpoint.error_count, 0);
        assert!(!endpoint.supports_responses_api);
        // デフォルトでChatCompletion機能を持つ
        assert!(endpoint.has_capability(EndpointCapability::ChatCompletion));
        assert!(!endpoint.has_capability(EndpointCapability::ImageGeneration));
    }

    #[test]
    fn test_endpoint_capability_serialization() {
        assert_eq!(
            serde_json::to_string(&EndpointCapability::ChatCompletion).unwrap(),
            "\"chat_completion\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointCapability::ImageGeneration).unwrap(),
            "\"image_generation\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointCapability::AudioTranscription).unwrap(),
            "\"audio_transcription\""
        );
    }

    #[test]
    fn test_supported_api_serialization() {
        // SupportedAPI列挙型のシリアライズテスト (SPEC-24157000)
        assert_eq!(
            serde_json::to_string(&SupportedAPI::ChatCompletions).unwrap(),
            "\"chat_completions\""
        );
        assert_eq!(
            serde_json::to_string(&SupportedAPI::Responses).unwrap(),
            "\"responses\""
        );
        assert_eq!(
            serde_json::to_string(&SupportedAPI::Embeddings).unwrap(),
            "\"embeddings\""
        );
    }

    #[test]
    fn test_supported_api_as_str() {
        assert_eq!(SupportedAPI::ChatCompletions.as_str(), "chat_completions");
        assert_eq!(SupportedAPI::Responses.as_str(), "responses");
        assert_eq!(SupportedAPI::Embeddings.as_str(), "embeddings");
    }

    #[test]
    fn test_endpoint_model_default_supported_apis() {
        // EndpointModelのデフォルトサポートAPI (SPEC-24157000)
        let json = r#"{"endpoint_id":"00000000-0000-0000-0000-000000000000","model_id":"test"}"#;
        let model: EndpointModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.supported_apis.len(), 1);
        assert_eq!(model.supported_apis[0], SupportedAPI::ChatCompletions);
    }

    #[test]
    fn test_endpoint_api_key_not_serialized() {
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        endpoint.api_key = Some("secret".to_string());

        let json = serde_json::to_string(&endpoint).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("api_key"));
    }

    // SPEC-f8e3a1b7: レイテンシ計算テスト (T021)

    #[test]
    fn test_update_inference_latency_initial() {
        // 初回更新: None → Some(value)
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        assert!(endpoint.inference_latency_ms.is_none());

        endpoint.update_inference_latency(100.0);
        assert_eq!(endpoint.inference_latency_ms, Some(100.0));
    }

    #[test]
    fn test_update_inference_latency_ema() {
        // EMA計算: α=0.2
        // new_ema = α * new + (1 - α) * old
        // new_ema = 0.2 * 200 + 0.8 * 100 = 40 + 80 = 120
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        endpoint.inference_latency_ms = Some(100.0);

        endpoint.update_inference_latency(200.0);
        assert_eq!(endpoint.inference_latency_ms, Some(120.0));

        // 続けて更新
        // new_ema = 0.2 * 100 + 0.8 * 120 = 20 + 96 = 116
        endpoint.update_inference_latency(100.0);
        assert_eq!(endpoint.inference_latency_ms, Some(116.0));
    }

    #[test]
    fn test_reset_inference_latency() {
        // オフライン時にINFINITYにリセット
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        endpoint.inference_latency_ms = Some(100.0);

        endpoint.reset_inference_latency();
        assert_eq!(endpoint.inference_latency_ms, Some(f64::INFINITY));
    }

    #[test]
    fn test_get_inference_latency_for_sort() {
        // None → INFINITY
        let endpoint1 = Endpoint::new("Test1".to_string(), "http://localhost:8080".to_string());
        assert_eq!(endpoint1.get_inference_latency_for_sort(), f64::INFINITY);

        // Some(value) → value
        let mut endpoint2 = Endpoint::new("Test2".to_string(), "http://localhost:8081".to_string());
        endpoint2.inference_latency_ms = Some(50.0);
        assert_eq!(endpoint2.get_inference_latency_for_sort(), 50.0);

        // Some(INFINITY) → INFINITY
        let mut endpoint3 = Endpoint::new("Test3".to_string(), "http://localhost:8082".to_string());
        endpoint3.reset_inference_latency();
        assert_eq!(endpoint3.get_inference_latency_for_sort(), f64::INFINITY);
    }

    #[test]
    fn test_update_inference_latency_from_infinity() {
        // INFINITY状態からの復帰: 新しい値がそのまま設定される
        let mut endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        endpoint.reset_inference_latency();
        assert_eq!(endpoint.inference_latency_ms, Some(f64::INFINITY));

        endpoint.update_inference_latency(100.0);
        assert_eq!(endpoint.inference_latency_ms, Some(100.0));
    }

    #[test]
    fn test_device_type_serialization() {
        assert_eq!(serde_json::to_string(&DeviceType::Cpu).unwrap(), "\"cpu\"");
        assert_eq!(serde_json::to_string(&DeviceType::Gpu).unwrap(), "\"gpu\"");
    }

    #[test]
    fn test_device_info_default() {
        let info = DeviceInfo::default();
        assert_eq!(info.device_type, DeviceType::Cpu);
        assert!(info.gpu_devices.is_empty());
    }

    #[test]
    fn test_device_info_serialization() {
        let info = DeviceInfo {
            device_type: DeviceType::Gpu,
            gpu_devices: vec![GpuDevice {
                name: "NVIDIA RTX 4090".to_string(),
                total_memory_bytes: 24_000_000_000,
                used_memory_bytes: 8_000_000_000,
            }],
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"device_type\":\"gpu\""));
        assert!(json.contains("NVIDIA RTX 4090"));

        let deserialized: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.device_type, DeviceType::Gpu);
        assert_eq!(deserialized.gpu_devices.len(), 1);
    }

    // SPEC-66555000: エンドポイントタイプ自動判別機能テスト

    #[test]
    fn test_endpoint_type_serialization() {
        assert_eq!(
            serde_json::to_string(&EndpointType::Xllm).unwrap(),
            "\"xllm\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointType::Ollama).unwrap(),
            "\"ollama\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointType::Vllm).unwrap(),
            "\"vllm\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointType::OpenaiCompatible).unwrap(),
            "\"openai_compatible\""
        );
        assert_eq!(
            serde_json::to_string(&EndpointType::Unknown).unwrap(),
            "\"unknown\""
        );
    }

    #[test]
    fn test_endpoint_type_from_str() {
        assert_eq!("xllm".parse::<EndpointType>().unwrap(), EndpointType::Xllm);
        assert_eq!(
            "ollama".parse::<EndpointType>().unwrap(),
            EndpointType::Ollama
        );
        assert_eq!("vllm".parse::<EndpointType>().unwrap(), EndpointType::Vllm);
        assert_eq!(
            "openai_compatible".parse::<EndpointType>().unwrap(),
            EndpointType::OpenaiCompatible
        );
        assert_eq!(
            "unknown".parse::<EndpointType>().unwrap(),
            EndpointType::Unknown
        );
        // 未知の値はUnknownにフォールバック
        assert_eq!(
            "invalid".parse::<EndpointType>().unwrap(),
            EndpointType::Unknown
        );
    }

    #[test]
    fn test_endpoint_type_as_str() {
        assert_eq!(EndpointType::Xllm.as_str(), "xllm");
        assert_eq!(EndpointType::Ollama.as_str(), "ollama");
        assert_eq!(EndpointType::Vllm.as_str(), "vllm");
        assert_eq!(EndpointType::OpenaiCompatible.as_str(), "openai_compatible");
        assert_eq!(EndpointType::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_endpoint_type_supports_model_download() {
        // xLLMのみダウンロードをサポート
        assert!(EndpointType::Xllm.supports_model_download());
        assert!(!EndpointType::Ollama.supports_model_download());
        assert!(!EndpointType::Vllm.supports_model_download());
        assert!(!EndpointType::OpenaiCompatible.supports_model_download());
        assert!(!EndpointType::Unknown.supports_model_download());
    }

    #[test]
    fn test_endpoint_type_supports_model_metadata() {
        // xLLMとOllamaがメタデータ取得をサポート
        assert!(EndpointType::Xllm.supports_model_metadata());
        assert!(EndpointType::Ollama.supports_model_metadata());
        assert!(!EndpointType::Vllm.supports_model_metadata());
        assert!(!EndpointType::OpenaiCompatible.supports_model_metadata());
        assert!(!EndpointType::Unknown.supports_model_metadata());
    }

    #[test]
    fn test_endpoint_type_default() {
        // デフォルトはUnknown
        assert_eq!(EndpointType::default(), EndpointType::Unknown);
    }

    #[test]
    fn test_download_status_serialization() {
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Downloading).unwrap(),
            "\"downloading\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&DownloadStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_download_status_from_str() {
        assert_eq!(
            "pending".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Pending
        );
        assert_eq!(
            "downloading".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Downloading
        );
        assert_eq!(
            "completed".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Completed
        );
        assert_eq!(
            "failed".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Failed
        );
        assert_eq!(
            "cancelled".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Cancelled
        );
        // 未知の値はPendingにフォールバック
        assert_eq!(
            "invalid".parse::<DownloadStatus>().unwrap(),
            DownloadStatus::Pending
        );
    }

    #[test]
    fn test_model_download_task_new() {
        let endpoint_id = Uuid::new_v4();
        let task = ModelDownloadTask::new(endpoint_id, "llama-3.2-1b".to_string());

        assert_eq!(task.endpoint_id, endpoint_id);
        assert_eq!(task.model, "llama-3.2-1b");
        assert_eq!(task.status, DownloadStatus::Pending);
        assert_eq!(task.progress, 0.0);
        assert!(task.filename.is_none());
        assert!(task.speed_mbps.is_none());
        assert!(task.eta_seconds.is_none());
        assert!(task.error_message.is_none());
        assert!(task.completed_at.is_none());
    }

    #[test]
    fn test_model_download_task_is_finished() {
        let endpoint_id = Uuid::new_v4();
        let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

        // 初期状態はPending -> 未完了
        assert!(!task.is_finished());

        // Downloading -> 未完了
        task.status = DownloadStatus::Downloading;
        assert!(!task.is_finished());

        // Completed -> 完了
        task.status = DownloadStatus::Completed;
        assert!(task.is_finished());

        // Failed -> 完了
        task.status = DownloadStatus::Failed;
        assert!(task.is_finished());

        // Cancelled -> 完了
        task.status = DownloadStatus::Cancelled;
        assert!(task.is_finished());
    }

    #[test]
    fn test_endpoint_new_has_endpoint_type() {
        let endpoint = Endpoint::new("Test".to_string(), "http://localhost:8080".to_string());
        // デフォルトはUnknown
        assert_eq!(endpoint.endpoint_type, EndpointType::Unknown);
    }

    #[test]
    fn test_endpoint_model_max_tokens() {
        let json = r#"{
            "endpoint_id": "00000000-0000-0000-0000-000000000000",
            "model_id": "test",
            "max_tokens": 4096
        }"#;
        let model: EndpointModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.max_tokens, Some(4096));
    }
}
