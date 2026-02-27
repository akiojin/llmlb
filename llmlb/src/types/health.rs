//! ヘルスメトリクス型定義
//!
//! エンドポイントのヘルスチェック・リクエスト関連の型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ヘルスメトリクス
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthMetrics {
    /// エンドポイントID
    pub endpoint_id: Uuid,
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
    /// 振り分け先エンドポイントID
    pub endpoint_id: Uuid,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    // --- 追加テスト ---

    #[test]
    fn test_request_status_serde_roundtrip() {
        for status in [
            RequestStatus::Pending,
            RequestStatus::Processing,
            RequestStatus::Completed,
            RequestStatus::Failed,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: RequestStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, status);
        }
    }

    #[test]
    fn test_health_metrics_serde_roundtrip() {
        let metrics = HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 45.5,
            memory_usage: 72.3,
            gpu_usage: Some(88.0),
            gpu_memory_usage: Some(65.2),
            gpu_memory_total_mb: Some(24576),
            gpu_memory_used_mb: Some(16000),
            gpu_temperature: Some(72.0),
            gpu_model_name: Some("NVIDIA RTX 4090".to_string()),
            gpu_compute_capability: Some("8.9".to_string()),
            gpu_capability_score: Some(100),
            active_requests: 5,
            total_requests: 1000,
            average_response_time_ms: Some(150.0),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: HealthMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cpu_usage, metrics.cpu_usage);
        assert_eq!(deserialized.memory_usage, metrics.memory_usage);
        assert_eq!(deserialized.gpu_usage, metrics.gpu_usage);
        assert_eq!(deserialized.active_requests, metrics.active_requests);
        assert_eq!(deserialized.total_requests, metrics.total_requests);
    }

    #[test]
    fn test_health_metrics_optional_fields_none() {
        let metrics = HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 10.0,
            memory_usage: 20.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&metrics).unwrap();
        // skip_serializing_if="Option::is_none" fields should be absent
        assert!(!json.contains("gpu_usage"));
        assert!(!json.contains("gpu_memory_usage"));
        assert!(!json.contains("gpu_temperature"));
        assert!(!json.contains("gpu_model_name"));
    }

    #[test]
    fn test_request_serde_roundtrip() {
        let request = Request {
            id: Uuid::new_v4(),
            endpoint_id: Uuid::new_v4(),
            endpoint: "/v1/chat/completions".to_string(),
            status: RequestStatus::Completed,
            duration_ms: Some(250),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.endpoint, "/v1/chat/completions");
        assert_eq!(deserialized.status, RequestStatus::Completed);
        assert_eq!(deserialized.duration_ms, Some(250));
    }

    #[test]
    fn test_request_pending_no_completion() {
        let request = Request {
            id: Uuid::new_v4(),
            endpoint_id: Uuid::new_v4(),
            endpoint: "/v1/embeddings".to_string(),
            status: RequestStatus::Pending,
            duration_ms: None,
            created_at: Utc::now(),
            completed_at: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, RequestStatus::Pending);
        assert!(deserialized.duration_ms.is_none());
        assert!(deserialized.completed_at.is_none());
    }
}
