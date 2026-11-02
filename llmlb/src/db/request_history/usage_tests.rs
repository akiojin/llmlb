use super::*;
use crate::common::usage::{UsageAttribution, UsageTokens};

fn sample() -> (UsageAttribution, RequestResponseRecord) {
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(Uuid::new_v4()),
        organization_id: Some(Uuid::new_v4()),
        received_at: Utc::now() - Duration::days(14),
        api_key_id: Some(Uuid::new_v4()),
        auth_source: "api_key".into(),
    };
    let record = RequestResponseRecord::new(
        Uuid::new_v4(),
        "endpoint".into(),
        "127.0.0.1".parse().unwrap(),
        "fixture".into(),
        RequestType::Chat,
        serde_json::json!({"model":"fixture"}),
        axum::http::StatusCode::OK,
        std::time::Duration::ZERO,
        Some("192.0.2.7".parse().unwrap()),
        attribution.api_key_id,
    );
    (attribution, record)
}

#[tokio::test]
async fn raw_receipt_and_final_tokens_share_once_transaction_and_immutable_identity() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let (attribution, mut record) = sample();
    storage.accept_usage(&attribution, &record).await.unwrap();
    let raw = storage
        .get_record_by_id(attribution.request_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.timestamp, attribution.received_at);
    assert!(storage.get_record_by_id(record.id).await.unwrap().is_none());
    record.response_body = Some(
        serde_json::json!({"usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}}),
    );
    storage
        .finish_usage(
            &attribution,
            &record,
            &UsageTokens::provider(Some(3), Some(2), Some(5)),
        )
        .await
        .unwrap();
    storage.accept_usage(&attribution, &record).await.unwrap();
    storage
        .finish_usage(
            &attribution,
            &record,
            &UsageTokens::provider(Some(99), Some(99), Some(198)),
        )
        .await
        .unwrap();
    let raw = storage
        .get_record_by_id(attribution.request_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.input_tokens, Some(3));
    assert_eq!(raw.total_tokens, Some(5));
    assert_eq!(raw.client_ip, Some("192.0.2.7".parse().unwrap()));
    let totals: (i64, i64) =
        sqlx::query_as("SELECT SUM(request_count),SUM(total_tokens) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(totals, (1, 5));
    storage
        .cleanup_old_records(Duration::days(7))
        .await
        .unwrap();
    storage.accept_usage(&attribution, &record).await.unwrap();
    storage
        .finish_usage(&attribution, &record, &UsageTokens::missing())
        .await
        .unwrap();
    assert!(storage
        .get_record_by_id(attribution.request_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT SUM(total_tokens) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        5
    );
}

#[tokio::test]
async fn raw_and_rollup_completion_roll_back_together_on_storage_failure() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let (attribution, record) = sample();
    storage.accept_usage(&attribution, &record).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_terminal BEFORE UPDATE ON request_history BEGIN SELECT RAISE(ABORT,'terminal failure'); END").execute(&pool).await.unwrap();
    assert!(storage
        .finish_usage(
            &attribution,
            &record,
            &UsageTokens::provider(Some(3), Some(2), Some(5))
        )
        .await
        .is_err());
    let state:(Option<String>,i64,i64)=sqlx::query_as("SELECT r.finalized_at,d.pending_requests,d.total_tokens FROM usage_receipts r JOIN usage_daily d ON d.date=r.date AND d.organization_id=r.organization_id AND d.account_id=r.account_id").fetch_one(&pool).await.unwrap();
    assert_eq!(state, (None, 1, 0));
    assert_eq!(
        storage
            .get_record_by_id(attribution.request_id)
            .await
            .unwrap()
            .unwrap()
            .total_tokens,
        None
    );
}

async fn drain_terminal_failure(storage: &RequestHistoryStorage, pool: &SqlitePool) {
    let drained = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        storage.drain_usage_writers(),
    )
    .await;
    if drained.is_err() {
        // A RED writer must not keep retrying against this test's open pool.
        pool.close().await;
    }
    assert!(
        drained.is_ok(),
        "permanent usage validation must not retry forever"
    );
    assert!(
        drained.unwrap().is_err(),
        "failed persistence must remain an error"
    );
}

#[tokio::test]
async fn permanent_terminal_validation_stops_retrying_and_survives_successful_writer_pruning() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let (failed, record) = sample();
    storage.accept_usage(&failed, &record).await.unwrap();
    storage.enqueue_usage_final(
        failed.clone(),
        record,
        UsageTokens::provider(Some(u64::MAX), None, None),
    );
    drain_terminal_failure(&storage, &pool).await;
    let unresolved: (Option<String>, i64, i64) = sqlx::query_as(
        "SELECT r.finalized_at,d.pending_requests,d.missing_count
         FROM usage_receipts r JOIN usage_daily d ON d.date=r.date
         AND d.organization_id=r.organization_id AND d.account_id=r.account_id
         WHERE r.request_id=?",
    )
    .bind(failed.request_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unresolved,
        (None, 1, 0),
        "failed writes stay incomplete, not measured zero"
    );

    let (healthy, record) = sample();
    storage.accept_usage(&healthy, &record).await.unwrap();
    storage.enqueue_usage_final(
        healthy.clone(),
        record,
        UsageTokens::provider(Some(2), Some(3), Some(5)),
    );
    // A second drain must finish healthy work while retaining the old failure.
    drain_terminal_failure(&storage, &pool).await;
    let healthy_finalized: Option<String> =
        sqlx::query_scalar("SELECT finalized_at FROM usage_receipts WHERE request_id=?")
            .bind(healthy.request_id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(healthy_finalized.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT SUM(total_tokens) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        5
    );
}

#[tokio::test]
async fn permanent_daily_overflow_returns_a_drain_error_and_preserves_atomic_pending_state() {
    let pool = crate::db::test_utils::test_db_pool().await;
    let storage = RequestHistoryStorage::new(pool.clone());
    let (first, record) = sample();
    let mut failed = first.clone();
    failed.request_id = Uuid::new_v4();
    storage.accept_usage(&first, &record).await.unwrap();
    storage.accept_usage(&failed, &record).await.unwrap();
    storage
        .finish_usage(
            &first,
            &record,
            &UsageTokens::provider(Some(i64::MAX as u64), None, None),
        )
        .await
        .unwrap();
    storage.enqueue_usage_final(
        failed.clone(),
        record,
        UsageTokens::provider(Some(1), None, None),
    );
    drain_terminal_failure(&storage, &pool).await;
    let state: (Option<String>, i64, i64, i64) = sqlx::query_as(
        "SELECT r.finalized_at,d.input_tokens,d.pending_requests,d.provider_count
         FROM usage_receipts r JOIN usage_daily d ON d.date=r.date
         AND d.organization_id=r.organization_id AND d.account_id=r.account_id
         WHERE r.request_id=?",
    )
    .bind(failed.request_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, (None, i64::MAX, 1, 1));
    let raw_input: Option<i64> =
        sqlx::query_scalar("SELECT input_tokens FROM request_history WHERE id=?")
            .bind(failed.request_id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        raw_input, None,
        "raw completion rolls back with the failed rollup"
    );
    assert_eq!(
        crate::db::usage::recover_incomplete(&pool).await.unwrap(),
        1
    );
    let recovered: (i64, i64, i64) =
        sqlx::query_as("SELECT input_tokens,pending_requests,missing_count FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        recovered,
        (i64::MAX, 0, 1),
        "startup recovery labels missing coverage explicitly"
    );
}

#[tokio::test]
async fn transient_database_busy_retries_until_raw_and_rollup_commit_once() {
    let directory = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(directory.path().join("usage-busy.db"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::ZERO);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let storage = RequestHistoryStorage::new(pool.clone());
    let (attribution, record) = sample();
    let tokens = UsageTokens::provider(Some(3), Some(2), Some(5));
    storage.accept_usage(&attribution, &record).await.unwrap();
    let transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let busy = storage.finish_usage(&attribution, &record, &tokens).await;
    assert!(
        busy.is_err(),
        "fixture must produce an actual SQLite write-lock failure"
    );
    storage.enqueue_usage_final(attribution.clone(), record, tokens);
    let drain = storage.drain_usage_writers();
    tokio::pin!(drain);
    tokio::task::yield_now().await;
    assert!(futures::poll!(drain.as_mut()).is_pending());
    transaction.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), drain)
        .await
        .unwrap()
        .unwrap();
    let state: (i64, i64, i64) =
        sqlx::query_as("SELECT request_count,pending_requests,total_tokens FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, (1, 0, 5));
    let raw = storage
        .get_record_by_id(attribution.request_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.total_tokens, Some(5));
}
