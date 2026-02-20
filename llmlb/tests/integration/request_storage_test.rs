//! ストレージ層の Integration Tests
//!
//! T007-T010: request_history.rs のストレージ機能をテスト

use chrono::{Duration, Utc};
use llmlb::common::protocol::{RecordStatus, RequestResponseRecord, RequestType};
use llmlb::db::request_history::{FilterStatus, RecordFilter, RequestHistoryStorage};
use std::net::IpAddr;
use uuid::Uuid;

async fn create_storage() -> RequestHistoryStorage {
    let pool = crate::support::lb::create_test_db_pool().await;
    RequestHistoryStorage::new(pool)
}

/// T007: 保存機能の integration test
#[tokio::test]
async fn test_save_record() {
    let storage = create_storage().await;
    let record = create_test_record("model-a", Uuid::new_v4(), Utc::now(), RecordStatus::Success);

    storage.save_record(&record).await.unwrap();

    let records = storage.load_records().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, record.id);
}

/// T007: 複数レコードの保存テスト
#[tokio::test]
async fn test_save_multiple_records() {
    let storage = create_storage().await;
    let record_a = create_test_record("model-a", Uuid::new_v4(), Utc::now(), RecordStatus::Success);
    let record_b = create_test_record("model-b", Uuid::new_v4(), Utc::now(), RecordStatus::Success);

    storage.save_record(&record_a).await.unwrap();
    storage.save_record(&record_b).await.unwrap();

    let records = storage.load_records().await.unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|r| r.id == record_a.id));
    assert!(records.iter().any(|r| r.id == record_b.id));
}

/// T008: 読み込み機能の integration test
#[tokio::test]
async fn test_load_records() {
    let storage = create_storage().await;
    let now = Utc::now();
    let older = now - Duration::minutes(5);

    let record_old = create_test_record("model-old", Uuid::new_v4(), older, RecordStatus::Success);
    let record_new = create_test_record("model-new", Uuid::new_v4(), now, RecordStatus::Success);

    storage.save_record(&record_old).await.unwrap();
    storage.save_record(&record_new).await.unwrap();

    let records = storage.load_records().await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].id, record_new.id);
    assert_eq!(records[1].id, record_old.id);
}

/// T008: ファイルが存在しない場合のテスト
#[tokio::test]
async fn test_load_records_file_not_exists() {
    let storage = create_storage().await;
    let records = storage.load_records().await.unwrap();
    assert!(records.is_empty());
}

/// T009: クリーンアップ機能の integration test
#[tokio::test]
async fn test_cleanup_old_records() {
    let storage = create_storage().await;
    let now = Utc::now();
    let old = now - Duration::days(8);
    let recent = now - Duration::days(1);

    let old_record = create_test_record("model-old", Uuid::new_v4(), old, RecordStatus::Success);
    let recent_record =
        create_test_record("model-new", Uuid::new_v4(), recent, RecordStatus::Success);

    storage.save_record(&old_record).await.unwrap();
    storage.save_record(&recent_record).await.unwrap();

    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();

    let records = storage.load_records().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, recent_record.id);
}

/// T009: クリーンアップの境界値テスト
#[tokio::test]
async fn test_cleanup_boundary() {
    let storage = create_storage().await;
    let now = Utc::now();
    let cutoff = now - Duration::days(7);
    let just_old = cutoff - Duration::seconds(2);
    let just_new = cutoff + Duration::seconds(2);

    let record_old =
        create_test_record("model-old", Uuid::new_v4(), just_old, RecordStatus::Success);
    let record_new =
        create_test_record("model-new", Uuid::new_v4(), just_new, RecordStatus::Success);

    storage.save_record(&record_old).await.unwrap();
    storage.save_record(&record_new).await.unwrap();

    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();

    let records = storage.load_records().await.unwrap();
    assert!(!records.iter().any(|r| r.id == record_old.id));
    assert!(records.iter().any(|r| r.id == record_new.id));
}

/// T010: フィルタリング機能の integration test
#[tokio::test]
async fn test_filter_by_model() {
    let storage = create_storage().await;
    let record_a = create_test_record(
        "model-alpha",
        Uuid::new_v4(),
        Utc::now(),
        RecordStatus::Success,
    );
    let record_b = create_test_record(
        "model-beta",
        Uuid::new_v4(),
        Utc::now(),
        RecordStatus::Success,
    );

    storage.save_record(&record_a).await.unwrap();
    storage.save_record(&record_b).await.unwrap();

    let filter = RecordFilter {
        model: Some("alpha".to_string()),
        ..Default::default()
    };
    let result = storage.filter_and_paginate(&filter, 1, 10).await.unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.records[0].id, record_a.id);
}

/// T010: ノードIDでフィルタ
#[tokio::test]
async fn test_filter_by_node_id() {
    let storage = create_storage().await;
    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    let record_a = create_test_record("model-a", node_a, Utc::now(), RecordStatus::Success);
    let record_b = create_test_record("model-b", node_b, Utc::now(), RecordStatus::Success);

    storage.save_record(&record_a).await.unwrap();
    storage.save_record(&record_b).await.unwrap();

    let filter = RecordFilter {
        node_id: Some(node_a),
        ..Default::default()
    };
    let result = storage.filter_and_paginate(&filter, 1, 10).await.unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.records[0].id, record_a.id);
}

/// T010: ステータスでフィルタ
#[tokio::test]
async fn test_filter_by_status() {
    let storage = create_storage().await;
    let success = create_test_record(
        "model-success",
        Uuid::new_v4(),
        Utc::now(),
        RecordStatus::Success,
    );
    let failure = create_test_record(
        "model-fail",
        Uuid::new_v4(),
        Utc::now(),
        RecordStatus::Error {
            message: "boom".to_string(),
        },
    );

    storage.save_record(&success).await.unwrap();
    storage.save_record(&failure).await.unwrap();

    let filter = RecordFilter {
        status: Some(FilterStatus::Success),
        ..Default::default()
    };
    let result = storage.filter_and_paginate(&filter, 1, 10).await.unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.records[0].id, success.id);
}

/// T010: 日時範囲でフィルタ
#[tokio::test]
async fn test_filter_by_time_range() {
    let storage = create_storage().await;
    let now = Utc::now();
    let early = now - Duration::hours(2);
    let mid = now - Duration::hours(1);
    let late = now - Duration::minutes(10);

    let record_early =
        create_test_record("model-early", Uuid::new_v4(), early, RecordStatus::Success);
    let record_mid = create_test_record("model-mid", Uuid::new_v4(), mid, RecordStatus::Success);
    let record_late = create_test_record("model-late", Uuid::new_v4(), late, RecordStatus::Success);

    storage.save_record(&record_early).await.unwrap();
    storage.save_record(&record_mid).await.unwrap();
    storage.save_record(&record_late).await.unwrap();

    let filter = RecordFilter {
        start_time: Some(mid),
        end_time: Some(now),
        ..Default::default()
    };
    let result = storage.filter_and_paginate(&filter, 1, 10).await.unwrap();
    assert_eq!(result.total_count, 2);
    assert!(result.records.iter().any(|r| r.id == record_mid.id));
    assert!(result.records.iter().any(|r| r.id == record_late.id));
}

/// T010: ページネーション
#[tokio::test]
async fn test_pagination() {
    let storage = create_storage().await;
    let now = Utc::now();
    let node_id = Uuid::new_v4();

    for i in 0..150 {
        let timestamp = now - Duration::seconds(i as i64);
        let record = create_test_record("model-page", node_id, timestamp, RecordStatus::Success);
        storage.save_record(&record).await.unwrap();
    }

    let filter = RecordFilter::default();
    let page1 = storage.filter_and_paginate(&filter, 1, 100).await.unwrap();
    assert_eq!(page1.records.len(), 100);
    assert_eq!(page1.total_count, 150);

    let page2 = storage.filter_and_paginate(&filter, 2, 100).await.unwrap();
    assert_eq!(page2.records.len(), 50);
    assert_eq!(page2.total_count, 150);
}

/// ヘルパー: テスト用のレコードを作成
fn create_test_record(
    model: &str,
    node_id: Uuid,
    timestamp: chrono::DateTime<Utc>,
    status: RecordStatus,
) -> RequestResponseRecord {
    RequestResponseRecord {
        id: Uuid::new_v4(),
        timestamp,
        request_type: RequestType::Chat,
        model: model.to_string(),
        node_id,
        node_machine_name: "test-node".to_string(),
        node_ip: "192.168.1.100".parse::<IpAddr>().unwrap(),
        client_ip: Some("10.0.0.10".parse::<IpAddr>().unwrap()),
        request_body: serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "test"}]
        }),
        response_body: Some(serde_json::json!({
            "message": {"role": "assistant", "content": "response"}
        })),
        duration_ms: 1000,
        status,
        completed_at: timestamp + Duration::seconds(1),
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        api_key_id: None,
    }
}
