use super::*;

#[tokio::test]
async fn test_archive_old_entries() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());

    // アーカイブ用インメモリDBを作成
    let archive_pool = super::create_archive_pool(":memory:").await.unwrap();

    // 古いエントリ（100日前）と新しいエントリ（1日前）を挿入
    let now = chrono::Utc::now();
    let old_time = now - chrono::Duration::days(100);
    let new_time = now - chrono::Duration::days(1);

    let entries = vec![
        AuditLogEntry {
            timestamp: old_time,
            ..make_entry("GET", "/api/old-1", 200, ActorType::User)
        },
        AuditLogEntry {
            timestamp: old_time - chrono::Duration::hours(1),
            ..make_entry("POST", "/api/old-2", 200, ActorType::User)
        },
        AuditLogEntry {
            timestamp: new_time,
            ..make_entry("GET", "/api/new-1", 200, ActorType::User)
        },
    ];
    storage.insert_batch(&entries).await.unwrap();

    // メインDBに3件あることを確認
    let main_count = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(main_count, 3);

    // アーカイブ実行（90日保持）
    let archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(archived, 2, "2 old entries should be archived");

    // メインDBに1件のみ残る
    let main_count = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(main_count, 1);

    // アーカイブDBに2件移動
    let archive_storage = AuditLogStorage::new(archive_pool.clone());
    let archive_count = archive_storage
        .count(&AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(archive_count, 2);

    // アーカイブDBからクエリ可能
    let archive_entries = storage
        .query_archive(&AuditLogFilter::default(), &archive_pool)
        .await
        .unwrap();
    assert_eq!(archive_entries.len(), 2);
}

#[tokio::test]
async fn test_archive_no_old_entries() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());

    let archive_pool = super::create_archive_pool(":memory:").await.unwrap();

    // 新しいエントリのみ
    let entries = vec![make_entry("GET", "/api/new", 200, ActorType::User)];
    storage.insert_batch(&entries).await.unwrap();

    // アーカイブ対象なし
    let archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(archived, 0);

    // メインDBに1件残る
    let main_count = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(main_count, 1);
}

#[tokio::test]
async fn test_archive_with_batch_hashes() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());

    let archive_pool = super::create_archive_pool(":memory:").await.unwrap();

    let now = chrono::Utc::now();
    let old_time = now - chrono::Duration::days(100);

    // バッチハッシュを作成
    let batch = AuditBatchHash {
        id: None,
        sequence_number: 1,
        batch_start: old_time - chrono::Duration::hours(1),
        batch_end: old_time,
        record_count: 1,
        hash: "abc123".to_string(),
        previous_hash: "0".repeat(64),
    };
    let batch_id = storage.insert_batch_hash(&batch).await.unwrap();

    // 古いエントリをバッチに関連付けて挿入
    let mut entry = make_entry("GET", "/api/old", 200, ActorType::User);
    entry.timestamp = old_time;
    storage.insert_batch(&[entry]).await.unwrap();

    // エントリのbatch_idを更新
    sqlx::query("UPDATE audit_log_entries SET batch_id = ? WHERE request_path = '/api/old'")
        .bind(batch_id)
        .execute(&pool)
        .await
        .unwrap();

    // アーカイブ実行
    let archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(archived, 1);

    // アーカイブDBにバッチハッシュもコピーされている
    let archive_batch: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM audit_batch_hashes WHERE id = ?")
            .bind(batch_id)
            .fetch_optional(&archive_pool)
            .await
            .unwrap();
    assert!(
        archive_batch.is_some(),
        "Batch hash should be copied to archive"
    );
}

#[tokio::test]
async fn test_archive_keeps_main_hash_chain_verifiable() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());
    let archive_pool = super::create_archive_pool(":memory:").await.unwrap();

    let now = chrono::Utc::now();
    let old_time = now - chrono::Duration::days(100);
    let new_time = now - chrono::Duration::days(1);

    let mut old_entry = make_entry("GET", "/api/old", 200, ActorType::User);
    old_entry.timestamp = old_time;
    let mut new_entry = make_entry("GET", "/api/new", 200, ActorType::User);
    new_entry.timestamp = new_time;
    storage
        .insert_batch(&[old_entry.clone(), new_entry.clone()])
        .await
        .unwrap();

    let old_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/old'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let new_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/new'")
            .fetch_one(&pool)
            .await
            .unwrap();

    old_entry.id = Some(old_id.0);
    new_entry.id = Some(new_id.0);

    let old_hash = crate::audit::hash_chain::compute_batch_hash(
        crate::audit::hash_chain::GENESIS_HASH,
        1,
        &old_time,
        &old_time,
        1,
        &[old_entry.clone()],
    );
    let old_batch_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: 1,
            batch_start: old_time,
            batch_end: old_time,
            record_count: 1,
            hash: old_hash.clone(),
            previous_hash: crate::audit::hash_chain::GENESIS_HASH.to_string(),
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[old_id.0], old_batch_id)
        .await
        .unwrap();

    let new_hash = crate::audit::hash_chain::compute_batch_hash(
        &old_hash,
        2,
        &new_time,
        &new_time,
        1,
        &[new_entry.clone()],
    );
    let new_batch_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: 2,
            batch_start: new_time,
            batch_end: new_time,
            record_count: 1,
            hash: new_hash,
            previous_hash: old_hash,
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[new_id.0], new_batch_id)
        .await
        .unwrap();

    let before = crate::audit::hash_chain::verify_chain(&storage)
        .await
        .unwrap();
    assert!(before.valid);
    assert_eq!(before.batches_checked, 2);

    let archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(archived, 1);

    let after = crate::audit::hash_chain::verify_chain(&storage)
        .await
        .unwrap();
    assert!(
        after.valid,
        "main DB hash chain must stay valid after archive"
    );
    assert_eq!(after.batches_checked, 1);

    let remaining_batches = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(remaining_batches.len(), 1);
    assert_eq!(remaining_batches[0].sequence_number, 2);
    assert_eq!(
        remaining_batches[0].previous_hash,
        crate::audit::hash_chain::GENESIS_HASH
    );
}

#[tokio::test]
async fn test_archive_multiple_runs_preserve_archive_hash_rows() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool.clone());
    let archive_pool = super::create_archive_pool(":memory:").await.unwrap();

    let now = chrono::Utc::now();
    let old_time_1 = now - chrono::Duration::days(200);
    let old_time_2 = now - chrono::Duration::days(120);
    let keep_time = now - chrono::Duration::days(1);

    let mut old_entry_1 = make_entry("GET", "/api/old-1", 200, ActorType::User);
    old_entry_1.timestamp = old_time_1;
    let mut old_entry_2 = make_entry("GET", "/api/old-2", 200, ActorType::User);
    old_entry_2.timestamp = old_time_2;
    let mut keep_entry = make_entry("GET", "/api/keep", 200, ActorType::User);
    keep_entry.timestamp = keep_time;
    storage
        .insert_batch(&[old_entry_1, old_entry_2, keep_entry])
        .await
        .unwrap();

    let old_1_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/old-1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let old_2_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/old-2'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let keep_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/keep'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let batch_1_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: 1,
            batch_start: old_time_1,
            batch_end: old_time_1,
            record_count: 1,
            hash: "hash-1".to_string(),
            previous_hash: crate::audit::hash_chain::GENESIS_HASH.to_string(),
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[old_1_id.0], batch_1_id)
        .await
        .unwrap();

    let batch_2_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: 2,
            batch_start: old_time_2,
            batch_end: old_time_2,
            record_count: 1,
            hash: "hash-2".to_string(),
            previous_hash: "hash-1".to_string(),
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[old_2_id.0], batch_2_id)
        .await
        .unwrap();

    let batch_3_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: 3,
            batch_start: keep_time,
            batch_end: keep_time,
            record_count: 1,
            hash: "hash-3".to_string(),
            previous_hash: "hash-2".to_string(),
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[keep_id.0], batch_3_id)
        .await
        .unwrap();

    let first_archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(first_archived, 2);

    let second_old_time = now - chrono::Duration::days(95);
    let mut second_old_entry = make_entry("GET", "/api/old-3", 200, ActorType::User);
    second_old_entry.timestamp = second_old_time;
    storage.insert_batch(&[second_old_entry]).await.unwrap();

    let old_3_id: (i64,) =
        sqlx::query_as("SELECT id FROM audit_log_entries WHERE request_path = '/api/old-3'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let latest_batch = storage.get_latest_batch_hash().await.unwrap().unwrap();
    let batch_4_id = storage
        .insert_batch_hash(&crate::audit::types::AuditBatchHash {
            id: None,
            sequence_number: latest_batch.sequence_number + 1,
            batch_start: second_old_time,
            batch_end: second_old_time,
            record_count: 1,
            hash: "hash-4".to_string(),
            previous_hash: latest_batch.hash,
        })
        .await
        .unwrap();
    storage
        .update_entries_batch_id(&[old_3_id.0], batch_4_id)
        .await
        .unwrap();

    let second_archived = storage
        .archive_old_entries(90, &archive_pool)
        .await
        .unwrap();
    assert_eq!(second_archived, 1);

    let missing_hash_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) \
             FROM audit_log_entries e \
             LEFT JOIN audit_batch_hashes b ON e.batch_id = b.id \
             WHERE e.batch_id IS NOT NULL AND b.id IS NULL",
    )
    .fetch_one(&archive_pool)
    .await
    .unwrap();
    assert_eq!(
        missing_hash_count.0, 0,
        "every archived batched entry must have a matching archive batch hash row"
    );
}
