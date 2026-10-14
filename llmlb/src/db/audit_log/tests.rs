use super::*;
use crate::audit::hash_chain::{self, GENESIS_HASH};
use crate::audit::types::ActorType;
use chrono::Utc;
use tempfile::TempDir;

mod archive_pool;
mod archive_run;
mod batch_hashes;
mod client_ip_filter;
mod fts;
mod migration;
mod query;
mod row_mapping;
mod token_statistics;
mod where_clause;

async fn create_test_pool() -> SqlitePool {
    crate::db::test_utils::test_db_pool().await
}

fn make_entry(method: &str, path: &str, status: u16, actor: ActorType) -> AuditLogEntry {
    AuditLogEntry {
        id: None,
        timestamp: Utc::now(),
        http_method: method.to_string(),
        request_path: path.to_string(),
        status_code: status,
        actor_type: actor,
        actor_id: Some("test-actor".to_string()),
        actor_username: Some("tester".to_string()),
        api_key_owner_id: None,
        client_ip: Some("127.0.0.1".to_string()),
        duration_ms: Some(42),
        input_tokens: Some(100),
        output_tokens: Some(50),
        total_tokens: Some(150),
        model_name: Some("test-model".to_string()),
        endpoint_id: Some("ep-1".to_string()),
        detail: None,
        batch_id: None,
        is_migrated: false,
    }
}

async fn prepare_legacy_archive() -> (TempDir, String, SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("audit-archive.db");
    let path = path.to_string_lossy().into_owned();
    let pool = create_archive_pool(&path).await.unwrap();
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_hash_chain_state (\
             id INTEGER PRIMARY KEY CHECK (id = 1), \
             algorithm_version INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT OR REPLACE INTO audit_hash_chain_state (id, algorithm_version) VALUES (1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    (directory, path, pool)
}

async fn insert_legacy_archive_batch(
    pool: &SqlitePool,
    sequence_number: i64,
    previous_hash: &str,
    entry: AuditLogEntry,
) -> String {
    let storage = AuditLogStorage::new(pool.clone());
    storage
        .insert_batch(std::slice::from_ref(&entry))
        .await
        .unwrap();
    let hash = hash_chain::compute_legacy_batch_hash(
        previous_hash,
        sequence_number,
        &entry.timestamp,
        &entry.timestamp,
        1,
        std::slice::from_ref(&entry),
    );
    let batch_id = storage
        .insert_batch_hash(&AuditBatchHash {
            id: None,
            sequence_number,
            batch_start: entry.timestamp,
            batch_end: entry.timestamp,
            record_count: 1,
            hash: hash.clone(),
            previous_hash: previous_hash.to_string(),
        })
        .await
        .unwrap();
    let entry_id: i64 =
        sqlx::query_scalar("SELECT id FROM audit_log_entries WHERE request_path = ?")
            .bind(&entry.request_path)
            .fetch_one(pool)
            .await
            .unwrap();
    storage
        .update_entries_batch_id(&[entry_id], batch_id)
        .await
        .unwrap();
    hash
}

async fn insert_current_archive_batch(
    pool: &SqlitePool,
    sequence_number: i64,
    previous_hash: &str,
    entry: AuditLogEntry,
) -> String {
    let storage = AuditLogStorage::new(pool.clone());
    storage
        .insert_batch(std::slice::from_ref(&entry))
        .await
        .unwrap();
    let hash = hash_chain::compute_batch_hash(
        previous_hash,
        sequence_number,
        &entry.timestamp,
        &entry.timestamp,
        1,
        std::slice::from_ref(&entry),
    );
    let batch_id = storage
        .insert_batch_hash(&AuditBatchHash {
            id: None,
            sequence_number,
            batch_start: entry.timestamp,
            batch_end: entry.timestamp,
            record_count: 1,
            hash: hash.clone(),
            previous_hash: previous_hash.to_string(),
        })
        .await
        .unwrap();
    let entry_id: i64 =
        sqlx::query_scalar("SELECT id FROM audit_log_entries WHERE request_path = ?")
            .bind(&entry.request_path)
            .fetch_one(pool)
            .await
            .unwrap();
    storage
        .update_entries_batch_id(&[entry_id], batch_id)
        .await
        .unwrap();
    hash
}

fn make_token_entry(
    model: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
    input: i64,
    output: i64,
    total: i64,
) -> AuditLogEntry {
    AuditLogEntry {
        id: None,
        timestamp,
        http_method: "POST".to_string(),
        request_path: "/v1/chat/completions".to_string(),
        status_code: 200,
        actor_type: ActorType::ApiKey,
        actor_id: Some("test-key".to_string()),
        actor_username: None,
        api_key_owner_id: None,
        client_ip: Some("127.0.0.1".to_string()),
        duration_ms: Some(100),
        input_tokens: Some(input),
        output_tokens: Some(output),
        total_tokens: Some(total),
        model_name: Some(model.to_string()),
        endpoint_id: Some("ep-1".to_string()),
        detail: None,
        batch_id: None,
        is_migrated: false,
    }
}

fn make_token_entry_without_total(
    model: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
    input: i64,
    output: i64,
) -> AuditLogEntry {
    let mut entry = make_token_entry(model, timestamp, input, output, 0);
    entry.total_tokens = None;
    entry
}
