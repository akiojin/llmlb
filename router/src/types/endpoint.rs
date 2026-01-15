//! エンドポイント型定義
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

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
