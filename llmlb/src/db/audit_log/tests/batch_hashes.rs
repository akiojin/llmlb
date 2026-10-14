use super::*;

#[tokio::test]
async fn test_insert_and_get_batch_hash() {
    use crate::audit::types::AuditBatchHash;

    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let batch = AuditBatchHash {
        id: None,
        sequence_number: 1,
        batch_start: now,
        batch_end: now,
        record_count: 10,
        hash: "abc123".to_string(),
        previous_hash: "0".repeat(64),
    };

    let id = storage.insert_batch_hash(&batch).await.unwrap();
    assert!(id > 0);

    let all = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, Some(id));
    assert_eq!(all[0].sequence_number, 1);
    assert_eq!(all[0].record_count, 10);
    assert_eq!(all[0].hash, "abc123");
    assert_eq!(all[0].previous_hash, "0".repeat(64));

    // 2つ目のバッチを追加
    let batch2 = AuditBatchHash {
        id: None,
        sequence_number: 2,
        batch_start: now,
        batch_end: now,
        record_count: 5,
        hash: "def456".to_string(),
        previous_hash: "abc123".to_string(),
    };
    storage.insert_batch_hash(&batch2).await.unwrap();

    let all = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(all.len(), 2);
    // ORDER BY sequence_number ASC
    assert_eq!(all[0].sequence_number, 1);
    assert_eq!(all[1].sequence_number, 2);
}

#[tokio::test]
async fn test_get_latest_batch_hash() {
    use crate::audit::types::AuditBatchHash;

    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    // 空の場合はNone
    let latest = storage.get_latest_batch_hash().await.unwrap();
    assert!(latest.is_none());

    let now = Utc::now();
    let batch1 = AuditBatchHash {
        id: None,
        sequence_number: 1,
        batch_start: now,
        batch_end: now,
        record_count: 10,
        hash: "hash1".to_string(),
        previous_hash: "0".repeat(64),
    };
    storage.insert_batch_hash(&batch1).await.unwrap();

    let batch2 = AuditBatchHash {
        id: None,
        sequence_number: 2,
        batch_start: now,
        batch_end: now,
        record_count: 5,
        hash: "hash2".to_string(),
        previous_hash: "hash1".to_string(),
    };
    storage.insert_batch_hash(&batch2).await.unwrap();

    let latest = storage.get_latest_batch_hash().await.unwrap();
    assert!(latest.is_some());
    let latest = latest.unwrap();
    assert_eq!(latest.sequence_number, 2);
    assert_eq!(latest.hash, "hash2");
}

#[tokio::test]
async fn test_get_entries_for_batch() {
    use crate::audit::types::AuditBatchHash;

    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    // エントリを挿入
    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("GET", "/health", 200, ActorType::Anonymous),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // バッチハッシュを挿入
    let now = Utc::now();
    let batch = AuditBatchHash {
        id: None,
        sequence_number: 1,
        batch_start: now,
        batch_end: now,
        record_count: 2,
        hash: "batchhash".to_string(),
        previous_hash: "0".repeat(64),
    };
    let batch_id = storage.insert_batch_hash(&batch).await.unwrap();

    // 最初の2エントリのbatch_idを更新
    let all = storage.query(&AuditLogFilter::default()).await.unwrap();
    let entry_ids: Vec<i64> = all.iter().take(2).filter_map(|e| e.id).collect();
    storage
        .update_entries_batch_id(&entry_ids, batch_id)
        .await
        .unwrap();

    // バッチ内エントリを取得
    let batch_entries = storage.get_entries_for_batch(batch_id).await.unwrap();
    assert_eq!(batch_entries.len(), 2);
    for entry in &batch_entries {
        assert_eq!(entry.batch_id, Some(batch_id));
    }

    // 存在しないバッチID
    let empty = storage.get_entries_for_batch(99999).await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_get_unbatched_entries() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());

    let entries = vec![
        make_entry("GET", "/api/a", 200, ActorType::User),
        make_entry("POST", "/api/b", 200, ActorType::ApiKey),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // All entries are unbatched initially
    let unbatched = storage.get_unbatched_entries().await.unwrap();
    assert_eq!(unbatched.len(), 2);

    // Assign one to a batch
    let batch = AuditBatchHash {
        id: None,
        sequence_number: 1,
        batch_start: Utc::now(),
        batch_end: Utc::now(),
        record_count: 1,
        hash: "test".to_string(),
        previous_hash: "0".repeat(64),
    };
    let batch_id = storage.insert_batch_hash(&batch).await.unwrap();

    let all = storage.query(&AuditLogFilter::default()).await.unwrap();
    let first_id = all[0].id.unwrap();
    storage
        .update_entries_batch_id(&[first_id], batch_id)
        .await
        .unwrap();

    let unbatched = storage.get_unbatched_entries().await.unwrap();
    assert_eq!(unbatched.len(), 1);
}

#[tokio::test]
async fn test_update_entries_batch_id_empty_ids() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    // Empty list should be a no-op
    storage.update_entries_batch_id(&[], 1).await.unwrap();
}

// =====================================================================
// 追加テスト: DB操作 - get_unbatched_entries empty
// =====================================================================

#[tokio::test]
async fn test_get_unbatched_entries_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let unbatched = storage.get_unbatched_entries().await.unwrap();
    assert!(unbatched.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - get_all_batch_hashes empty
// =====================================================================

#[tokio::test]
async fn test_get_all_batch_hashes_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let hashes = storage.get_all_batch_hashes().await.unwrap();
    assert!(hashes.is_empty());
}
