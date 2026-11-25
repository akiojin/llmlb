//! リクエスト関連の型定義
//!
//! エンドポイントへ振り分けたリクエストの表現型。
//! 旧 HealthMetrics（heartbeat で受け取るエンドポイント内部状態）は
//! Issue #982 で heartbeat 受付ごと撤去した。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
