//! エンドポイント型定義
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

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
    /// 表示名（例: "本番Ollama", "開発aLLM1"）
    pub name: String,
    /// ベースURL（例: `http://192.168.1.100:11434`）
    pub base_url: String,
    /// APIキー（暗号化保存、シリアライズ時はスキップ）
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    /// 現在の状態
    pub status: EndpointStatus,
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
    /// GPU情報（/v0/healthから取得、Phase 1.4）
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
        }
    }

    /// 指定した機能をサポートしているか確認
    pub fn has_capability(&self, cap: EndpointCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// EndpointからNodeへの変換（NodeRegistry廃止移行用）
    ///
    /// LoadManagerのロードバランシングロジックを再利用するための一時的な変換。
    /// EndpointRegistry完全移行後に削除予定。
    #[deprecated(
        note = "This is a temporary bridge for NodeRegistry migration. Will be removed after full EndpointRegistry migration."
    )]
    #[allow(deprecated)] // Uses deprecated Node type for migration bridge
    pub fn to_legacy_node(&self, models: Vec<String>) -> llm_router_common::types::Node {
        use llm_router_common::types::{Node, NodeStatus};
        use std::collections::HashSet;
        use std::net::IpAddr;

        // base_urlからホストとポートを抽出
        let url = reqwest::Url::parse(&self.base_url).ok();
        let host = url
            .as_ref()
            .and_then(|u| u.host_str())
            .unwrap_or("127.0.0.1");
        let port = url.as_ref().and_then(|u| u.port()).unwrap_or(8080);

        // IPアドレスをパース（失敗時はローカルホスト）
        let ip_address: IpAddr = host.parse().unwrap_or_else(|_| {
            // ホスト名の場合はDNS解決が必要だが、ここでは127.0.0.1にフォールバック
            "127.0.0.1".parse().unwrap()
        });

        // EndpointStatusからNodeStatusへ変換
        let status = match self.status {
            EndpointStatus::Online => NodeStatus::Online,
            EndpointStatus::Pending => NodeStatus::Pending,
            _ => NodeStatus::Offline,
        };

        Node {
            id: self.id,
            machine_name: self.name.clone(),
            ip_address,
            runtime_version: String::new(), // Endpointには相当フィールドなし
            runtime_port: port.saturating_sub(1), // OpenAI APIポート-1 = runtimeポート（慣例）
            status,
            registered_at: self.registered_at,
            last_seen: self.last_seen.unwrap_or(self.registered_at),
            online_since: if self.status == EndpointStatus::Online {
                Some(self.last_seen.unwrap_or(self.registered_at))
            } else {
                None
            },
            custom_name: Some(self.name.clone()),
            tags: vec![],
            notes: self.notes.clone(),
            loaded_models: models.clone(),
            loaded_embedding_models: vec![],
            loaded_asr_models: vec![],
            loaded_tts_models: vec![],
            executable_models: models,
            excluded_models: HashSet::new(),
            supported_runtimes: vec![],
            gpu_devices: vec![],
            gpu_available: false,
            gpu_count: None,
            gpu_model: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            node_api_port: Some(port),
            initializing: false,
            ready_models: None,
            sync_state: None,
            sync_progress: None,
            sync_updated_at: None,
        }
    }
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
}
