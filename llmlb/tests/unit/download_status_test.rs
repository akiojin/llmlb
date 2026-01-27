//! SPEC-66555000: DownloadStatus状態遷移のUnit Test
//!
//! T140: DownloadStatus遷移のunit test

use llmlb::types::endpoint::{DownloadStatus, ModelDownloadTask};
use serde_json;
use uuid::Uuid;

/// 状態遷移: Pending → Downloading → Completed
#[test]
fn test_state_transition_success() {
    let endpoint_id = Uuid::new_v4();
    let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    // 初期状態: Pending
    assert_eq!(task.status, DownloadStatus::Pending);
    assert!(!task.is_finished());

    // Downloading
    task.status = DownloadStatus::Downloading;
    assert!(!task.is_finished());

    // Completed
    task.status = DownloadStatus::Completed;
    assert!(task.is_finished());
}

/// 状態遷移: Pending → Downloading → Failed
#[test]
fn test_state_transition_failure() {
    let endpoint_id = Uuid::new_v4();
    let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    task.status = DownloadStatus::Downloading;
    assert!(!task.is_finished());

    task.status = DownloadStatus::Failed;
    task.error_message = Some("Download failed".to_string());
    assert!(task.is_finished());
}

/// 状態遷移: Pending → Cancelled
#[test]
fn test_state_transition_cancelled() {
    let endpoint_id = Uuid::new_v4();
    let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    task.status = DownloadStatus::Cancelled;
    assert!(task.is_finished());
}

/// is_finished: 終了状態の判定
#[test]
fn test_is_finished() {
    let endpoint_id = Uuid::new_v4();
    let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    // 進行中状態: 未完了
    task.status = DownloadStatus::Pending;
    assert!(!task.is_finished());

    task.status = DownloadStatus::Downloading;
    assert!(!task.is_finished());

    // 終了状態: 完了
    task.status = DownloadStatus::Completed;
    assert!(task.is_finished());

    task.status = DownloadStatus::Failed;
    assert!(task.is_finished());

    task.status = DownloadStatus::Cancelled;
    assert!(task.is_finished());
}

/// JSON シリアライズ: snake_case形式
#[test]
fn test_json_serialization() {
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

/// JSON デシリアライズ
#[test]
fn test_json_deserialization() {
    assert_eq!(
        serde_json::from_str::<DownloadStatus>("\"pending\"").unwrap(),
        DownloadStatus::Pending
    );
    assert_eq!(
        serde_json::from_str::<DownloadStatus>("\"downloading\"").unwrap(),
        DownloadStatus::Downloading
    );
    assert_eq!(
        serde_json::from_str::<DownloadStatus>("\"completed\"").unwrap(),
        DownloadStatus::Completed
    );
    assert_eq!(
        serde_json::from_str::<DownloadStatus>("\"failed\"").unwrap(),
        DownloadStatus::Failed
    );
    assert_eq!(
        serde_json::from_str::<DownloadStatus>("\"cancelled\"").unwrap(),
        DownloadStatus::Cancelled
    );
}

/// FromStr: 正常系
#[test]
fn test_from_str_valid() {
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
}

/// FromStr: 不正値はPendingにフォールバック
#[test]
fn test_from_str_invalid_fallback() {
    assert_eq!(
        "invalid".parse::<DownloadStatus>().unwrap(),
        DownloadStatus::Pending
    );
    assert_eq!(
        "".parse::<DownloadStatus>().unwrap(),
        DownloadStatus::Pending
    );
    assert_eq!(
        "PENDING".parse::<DownloadStatus>().unwrap(),
        DownloadStatus::Pending
    );
}

/// as_str: 各バリアントの文字列表現
#[test]
fn test_as_str() {
    assert_eq!(DownloadStatus::Pending.as_str(), "pending");
    assert_eq!(DownloadStatus::Downloading.as_str(), "downloading");
    assert_eq!(DownloadStatus::Completed.as_str(), "completed");
    assert_eq!(DownloadStatus::Failed.as_str(), "failed");
    assert_eq!(DownloadStatus::Cancelled.as_str(), "cancelled");
}

/// Display: as_str()と一致
#[test]
fn test_display() {
    let statuses = [
        DownloadStatus::Pending,
        DownloadStatus::Downloading,
        DownloadStatus::Completed,
        DownloadStatus::Failed,
        DownloadStatus::Cancelled,
    ];

    for s in statuses {
        assert_eq!(format!("{}", s), s.as_str());
    }
}

/// Default: Pending
#[test]
fn test_default() {
    let default_status: DownloadStatus = Default::default();
    assert_eq!(default_status, DownloadStatus::Pending);
}

/// ModelDownloadTask: 初期化
#[test]
fn test_task_new() {
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
    assert!(!task.id.is_empty());
}

/// ModelDownloadTask: 進捗更新
#[test]
fn test_task_progress_update() {
    let endpoint_id = Uuid::new_v4();
    let mut task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    task.status = DownloadStatus::Downloading;
    task.progress = 0.5;
    task.speed_mbps = Some(100.0);
    task.eta_seconds = Some(60);
    task.filename = Some("model.gguf".to_string());

    assert_eq!(task.status, DownloadStatus::Downloading);
    assert_eq!(task.progress, 0.5);
    assert_eq!(task.speed_mbps, Some(100.0));
    assert_eq!(task.eta_seconds, Some(60));
    assert_eq!(task.filename, Some("model.gguf".to_string()));
}

/// ModelDownloadTask: JSON ラウンドトリップ
#[test]
fn test_task_json_roundtrip() {
    let endpoint_id = Uuid::new_v4();
    let task = ModelDownloadTask::new(endpoint_id, "test-model".to_string());

    let json = serde_json::to_string(&task).unwrap();
    let parsed: ModelDownloadTask = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.endpoint_id, task.endpoint_id);
    assert_eq!(parsed.model, task.model);
    assert_eq!(parsed.status, task.status);
}
