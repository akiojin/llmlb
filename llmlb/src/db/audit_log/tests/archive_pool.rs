use super::*;

#[tokio::test]
async fn test_create_archive_pool_rehashes_legacy_batches_once() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-legacy", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let migrated_pool = create_archive_pool(&path).await.unwrap();
    let storage = AuditLogStorage::new(migrated_pool.clone());
    assert!(hash_chain::verify_chain(&storage).await.unwrap().valid);

    let migrated_hash = storage.get_all_batch_hashes().await.unwrap()[0]
        .hash
        .clone();
    sqlx::query(
        "UPDATE audit_log_entries SET client_ip = '198.51.100.9' \
             WHERE request_path = '/api/archive-legacy'",
    )
    .execute(&migrated_pool)
    .await
    .unwrap();
    migrated_pool.close().await;

    let reopened_pool = create_archive_pool(&path).await.unwrap();
    let reopened = AuditLogStorage::new(reopened_pool);
    assert_eq!(
        reopened.get_all_batch_hashes().await.unwrap()[0].hash,
        migrated_hash,
        "a later archive open must not rebaseline tampered data"
    );
    assert!(!hash_chain::verify_chain(&reopened).await.unwrap().valid);
}

#[tokio::test]
async fn test_create_archive_pool_accepts_external_anchor_for_first_batch() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        4,
        &"a".repeat(64),
        make_entry("GET", "/api/archive-4", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let migrated_pool = create_archive_pool(&path).await.unwrap();
    let storage = AuditLogStorage::new(migrated_pool);
    let batches = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].previous_hash, "a".repeat(64));
    let entries = storage
        .get_entries_for_batch(batches[0].id.unwrap())
        .await
        .unwrap();
    assert_eq!(
        batches[0].hash,
        hash_chain::compute_batch_hash(
            &batches[0].previous_hash,
            batches[0].sequence_number,
            &batches[0].batch_start,
            &batches[0].batch_end,
            batches[0].record_count,
            &entries,
        )
    );
}

#[tokio::test]
async fn test_create_archive_pool_rehashes_mixed_legacy_and_current_batches() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    let legacy_hash = insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-legacy", 200, ActorType::User),
    )
    .await;
    insert_current_archive_batch(
        &pool,
        2,
        &legacy_hash,
        make_entry("GET", "/api/archive-current", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let migrated_pool = create_archive_pool(&path).await.unwrap();
    let storage = AuditLogStorage::new(migrated_pool);
    assert!(hash_chain::verify_chain(&storage).await.unwrap().valid);
}

#[tokio::test]
async fn test_create_archive_pool_rehashes_mixed_batches_across_segments() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-legacy", 200, ActorType::User),
    )
    .await;
    insert_current_archive_batch(
        &pool,
        4,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-current", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let migrated_pool = create_archive_pool(&path).await.unwrap();
    let storage = AuditLogStorage::new(migrated_pool);
    let batches = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(batches.len(), 2);
    for batch in batches {
        let entries = storage
            .get_entries_for_batch(batch.id.unwrap())
            .await
            .unwrap();
        assert_eq!(
            batch.hash,
            hash_chain::compute_batch_hash(
                &batch.previous_hash,
                batch.sequence_number,
                &batch.batch_start,
                &batch.batch_end,
                batch.record_count,
                &entries,
            )
        );
    }
}

#[tokio::test]
async fn test_create_archive_pool_rehashes_consecutive_rebased_segments() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-first-run", 200, ActorType::User),
    )
    .await;
    insert_legacy_archive_batch(
        &pool,
        2,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-second-run", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let migrated_pool = create_archive_pool(&path).await.unwrap();
    let storage = AuditLogStorage::new(migrated_pool);
    let batches = storage.get_all_batch_hashes().await.unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[1].previous_hash, GENESIS_HASH);
    for batch in batches {
        let entries = storage
            .get_entries_for_batch(batch.id.unwrap())
            .await
            .unwrap();
        assert_eq!(
            batch.hash,
            hash_chain::compute_batch_hash(
                &batch.previous_hash,
                batch.sequence_number,
                &batch.batch_start,
                &batch.batch_end,
                batch.record_count,
                &entries,
            )
        );
    }
}

#[tokio::test]
async fn test_create_archive_pool_creates_batch_index() {
    let pool = create_archive_pool(":memory:").await.unwrap();

    let index_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'index' AND tbl_name = 'audit_log_entries' \
             AND name = 'idx_archive_batch'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(index_count, 1);
}

#[tokio::test]
async fn test_create_archive_pool_rejects_invalid_external_anchor() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        4,
        "not-a-sha256",
        make_entry("GET", "/api/archive-invalid-anchor", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let error = create_archive_pool(&path)
        .await
        .expect_err("an invalid external anchor must fail closed");
    assert!(error.to_string().contains("audit hash chain"));
}

#[tokio::test]
async fn test_create_archive_pool_rejects_deleted_middle_batch() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    let hash_1 = insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-1", 200, ActorType::User),
    )
    .await;
    let hash_2 = insert_legacy_archive_batch(
        &pool,
        2,
        &hash_1,
        make_entry("GET", "/api/archive-2", 200, ActorType::User),
    )
    .await;
    insert_legacy_archive_batch(
        &pool,
        3,
        &hash_2,
        make_entry("GET", "/api/archive-3", 200, ActorType::User),
    )
    .await;

    let deleted_batch_id: i64 =
        sqlx::query_scalar("SELECT id FROM audit_batch_hashes WHERE sequence_number = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM audit_log_entries WHERE batch_id = ?")
        .bind(deleted_batch_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM audit_batch_hashes WHERE id = ?")
        .bind(deleted_batch_id)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let error = create_archive_pool(&path)
        .await
        .expect_err("a deleted middle batch must fail closed");
    assert!(error.to_string().contains("invalid external predecessor"));
}

#[tokio::test]
async fn test_create_archive_pool_rejects_internal_predecessor_skip() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    let hash_1 = insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-1", 200, ActorType::User),
    )
    .await;
    insert_legacy_archive_batch(
        &pool,
        2,
        &hash_1,
        make_entry("GET", "/api/archive-2", 200, ActorType::User),
    )
    .await;
    insert_legacy_archive_batch(
        &pool,
        3,
        &hash_1,
        make_entry("GET", "/api/archive-3", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let error = create_archive_pool(&path)
        .await
        .expect_err("an internal predecessor skip must fail closed");
    assert!(error.to_string().contains("audit hash chain"));
}

#[tokio::test]
async fn test_create_archive_pool_rejects_contiguous_external_predecessor() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-1", 200, ActorType::User),
    )
    .await;
    insert_legacy_archive_batch(
        &pool,
        2,
        &"b".repeat(64),
        make_entry("GET", "/api/archive-2", 200, ActorType::User),
    )
    .await;
    pool.close().await;

    let error = create_archive_pool(&path)
        .await
        .expect_err("a contiguous batch must reference its immediate predecessor");
    assert!(error.to_string().contains("audit hash chain"));
}

#[tokio::test]
async fn test_create_archive_pool_rejects_tampered_legacy_batch() {
    let (_directory, path, pool) = prepare_legacy_archive().await;
    insert_legacy_archive_batch(
        &pool,
        1,
        GENESIS_HASH,
        make_entry("GET", "/api/archive-legacy", 200, ActorType::User),
    )
    .await;
    sqlx::query(
        "UPDATE audit_log_entries SET request_path = '/api/tampered' \
             WHERE request_path = '/api/archive-legacy'",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let error = create_archive_pool(&path)
        .await
        .expect_err("tampered archive must fail closed");
    assert!(error.to_string().contains("audit hash chain"));
}
