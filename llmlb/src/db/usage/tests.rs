use crate::common::usage::{TokenSource, UsageAttribution, UsageTokens};
use crate::db::usage;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use uuid::Uuid;

fn attribution(time: &str) -> UsageAttribution {
    UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id: Some(Uuid::new_v4()),
        organization_id: Some(Uuid::new_v4()),
        received_at: time.parse::<DateTime<Utc>>().unwrap(),
        api_key_id: Some(Uuid::new_v4()),
        auth_source: "api_key".to_string(),
    }
}

async fn pool() -> SqlitePool {
    crate::db::test_utils::test_db_pool().await
}

async fn totals(pool: &SqlitePool) -> sqlx::sqlite::SqliteRow {
    sqlx::query(
        "SELECT COALESCE(SUM(request_count), 0) AS requests,
                COALESCE(SUM(pending_requests), 0) AS pending,
                COALESCE(SUM(input_tokens), 0) AS input,
                COALESCE(SUM(output_tokens), 0) AS output,
                COALESCE(SUM(total_tokens), 0) AS total,
                COALESCE(SUM(provider_count), 0) AS provider,
                COALESCE(SUM(estimated_count), 0) AS estimated,
                COALESCE(SUM(missing_count), 0) AS missing,
                COALESCE(SUM(input_missing_count), 0) AS input_missing,
                COALESCE(SUM(output_missing_count), 0) AS output_missing,
                COALESCE(SUM(total_missing_count), 0) AS total_missing
         FROM usage_daily",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn accepts_and_finishes_one_logical_request_once() {
    let pool = pool().await;
    let accepted = attribution("2026-10-31T23:59:59.900Z");
    usage::accept(&pool, &accepted).await.unwrap();
    usage::accept(&pool, &accepted).await.unwrap();
    let pending = totals(&pool).await;
    assert_eq!(pending.get::<i64, _>("requests"), 1);
    assert_eq!(pending.get::<i64, _>("pending"), 1);

    let tokens = UsageTokens::provider(Some(12), Some(7), Some(19));
    usage::finish(&pool, accepted.request_id, &tokens)
        .await
        .unwrap();
    usage::finish(&pool, accepted.request_id, &tokens)
        .await
        .unwrap();
    usage::accept(&pool, &accepted).await.unwrap();
    let row = totals(&pool).await;
    assert_eq!(row.get::<i64, _>("requests"), 1);
    assert_eq!(row.get::<i64, _>("pending"), 0);
    assert_eq!(row.get::<i64, _>("input"), 12);
    assert_eq!(row.get::<i64, _>("output"), 7);
    assert_eq!(row.get::<i64, _>("total"), 19);
    assert_eq!(row.get::<i64, _>("provider"), 1);
    assert_eq!(row.get::<i64, _>("missing"), 0);
    let date: String = sqlx::query_scalar("SELECT date FROM usage_daily")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        date, "2026-10-31",
        "completion never changes the UTC receipt bucket"
    );
}

#[tokio::test]
async fn duplicate_accept_cannot_replace_any_attribution_field() {
    let pool = pool().await;
    let original = attribution("2026-10-31T23:59:59Z");
    usage::accept(&pool, &original).await.unwrap();
    let mut replacements = Vec::new();
    let mut changed = original.clone();
    changed.account_id = Some(Uuid::new_v4());
    replacements.push(changed);
    let mut changed = original.clone();
    changed.organization_id = Some(Uuid::new_v4());
    replacements.push(changed);
    let mut changed = original.clone();
    changed.received_at = "2026-11-01T00:00:00Z".parse().unwrap();
    replacements.push(changed);
    let mut changed = original.clone();
    changed.api_key_id = Some(Uuid::new_v4());
    replacements.push(changed);
    let mut changed = original.clone();
    changed.auth_source = "jwt".to_string();
    replacements.push(changed);
    for replacement in replacements {
        assert!(usage::accept(&pool, &replacement).await.is_err());
    }
    usage::finish(
        &pool,
        original.request_id,
        &UsageTokens::provider(Some(3), Some(4), Some(7)),
    )
    .await
    .unwrap();
    let snapshot = sqlx::query(
        "SELECT account_id, organization_id, received_at, api_key_id, auth_source
         FROM usage_receipts WHERE request_id = ?",
    )
    .bind(original.request_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        snapshot.get::<String, _>("account_id"),
        original.account_id.unwrap().to_string()
    );
    assert_eq!(
        snapshot.get::<String, _>("organization_id"),
        original.organization_id.unwrap().to_string()
    );
    assert_eq!(
        snapshot.get::<String, _>("api_key_id"),
        original.api_key_id.unwrap().to_string()
    );
    assert_eq!(snapshot.get::<String, _>("auth_source"), "api_key");
    assert_eq!(totals(&pool).await.get::<i64, _>("requests"), 1);
}

#[tokio::test]
async fn measured_zero_estimates_partial_coverage_and_missing_are_distinct() {
    let pool = pool().await;
    let cases = [
        UsageTokens::provider(Some(0), Some(0), Some(0)),
        UsageTokens::estimated(Some(4), Some(3), Some(7)),
        UsageTokens::provider(Some(9), None, None),
        UsageTokens::missing(),
    ];
    for tokens in cases {
        let request = attribution("2026-11-01T00:00:00Z");
        usage::accept(&pool, &request).await.unwrap();
        usage::finish(&pool, request.request_id, &tokens)
            .await
            .unwrap();
    }
    let row = totals(&pool).await;
    assert_eq!(row.get::<i64, _>("requests"), 4);
    assert_eq!(row.get::<i64, _>("input"), 13);
    assert_eq!(row.get::<i64, _>("output"), 3);
    assert_eq!(
        row.get::<i64, _>("total"),
        7,
        "missing total is never inferred from partial provider usage"
    );
    assert_eq!(row.get::<i64, _>("provider"), 2);
    assert_eq!(row.get::<i64, _>("estimated"), 1);
    assert_eq!(row.get::<i64, _>("missing"), 1);
    assert_eq!(row.get::<i64, _>("input_missing"), 1);
    assert_eq!(row.get::<i64, _>("output_missing"), 2);
    assert_eq!(row.get::<i64, _>("total_missing"), 2);
    assert_eq!(row.get::<i64, _>("pending"), 0);
}

#[tokio::test]
async fn unknown_completion_and_unrepresentable_tokens_fail_without_finalizing() {
    let pool = pool().await;
    let accepted = attribution("2026-11-01T00:00:00Z");
    assert!(
        usage::finish(&pool, accepted.request_id, &UsageTokens::missing())
            .await
            .is_err()
    );
    usage::accept(&pool, &accepted).await.unwrap();
    let invalid = [
        UsageTokens::provider(Some(u64::MAX), None, None),
        UsageTokens::provider(None, Some(u64::MAX), None),
        UsageTokens::provider(None, None, Some(u64::MAX)),
        UsageTokens {
            input_tokens: Some(1),
            output_tokens: None,
            total_tokens: None,
            source: TokenSource::Missing,
        },
    ];
    for tokens in invalid {
        assert!(usage::finish(&pool, accepted.request_id, &tokens)
            .await
            .is_err());
    }
    let before = totals(&pool).await;
    assert_eq!(before.get::<i64, _>("pending"), 1);
    assert_eq!(before.get::<i64, _>("input"), 0);
    assert_eq!(before.get::<i64, _>("missing"), 0);
    usage::finish(&pool, accepted.request_id, &UsageTokens::missing())
        .await
        .unwrap();
    assert_eq!(totals(&pool).await.get::<i64, _>("missing"), 1);
}

#[tokio::test]
async fn usage_can_share_and_roll_back_the_raw_history_transaction() {
    let pool = pool().await;
    let accepted = attribution("2026-11-01T00:00:00Z");
    let mut transaction = pool.begin().await.unwrap();
    usage::accept_in_transaction(&mut transaction, &accepted)
        .await
        .unwrap();
    usage::finish_in_transaction(
        &mut transaction,
        accepted.request_id,
        &UsageTokens::provider(Some(5), Some(8), Some(13)),
    )
    .await
    .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(totals(&pool).await.get::<i64, _>("requests"), 0);
    let receipts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
}

#[tokio::test]
async fn daily_integer_overflow_rolls_back_the_completion_atomically() {
    let pool = pool().await;
    let first = attribution("2026-11-01T00:00:00Z");
    let mut second = first.clone();
    second.request_id = Uuid::new_v4();
    usage::accept(&pool, &first).await.unwrap();
    usage::accept(&pool, &second).await.unwrap();
    usage::finish(
        &pool,
        first.request_id,
        &UsageTokens::provider(Some(i64::MAX as u64), None, None),
    )
    .await
    .unwrap();
    assert!(usage::finish(
        &pool,
        second.request_id,
        &UsageTokens::provider(Some(1), None, None),
    )
    .await
    .is_err());
    let row = totals(&pool).await;
    assert_eq!(row.get::<i64, _>("input"), i64::MAX);
    assert_eq!(row.get::<i64, _>("pending"), 1);
    assert_eq!(row.get::<i64, _>("provider"), 1);
    usage::finish(&pool, second.request_id, &UsageTokens::missing())
        .await
        .unwrap();
    assert_eq!(totals(&pool).await.get::<i64, _>("pending"), 0);
}

#[tokio::test]
async fn concurrent_duplicate_callbacks_and_different_final_values_apply_only_once() {
    let pool = pool().await;
    let accepted = attribution("2026-11-01T00:00:00Z");
    let (first, second) = tokio::join!(
        usage::accept(&pool, &accepted),
        usage::accept(&pool, &accepted)
    );
    first.unwrap();
    second.unwrap();
    let tokens = UsageTokens::provider(Some(11), Some(2), Some(13));
    let (first, second) = tokio::join!(
        usage::finish(&pool, accepted.request_id, &tokens),
        usage::finish(&pool, accepted.request_id, &tokens)
    );
    first.unwrap();
    second.unwrap();
    usage::finish(
        &pool,
        accepted.request_id,
        &UsageTokens::provider(Some(99), Some(99), Some(198)),
    )
    .await
    .unwrap();
    let row = totals(&pool).await;
    assert_eq!(row.get::<i64, _>("requests"), 1);
    assert_eq!(row.get::<i64, _>("input"), 11);
    assert_eq!(row.get::<i64, _>("total"), 13);
}

#[tokio::test]
async fn raw_cleanup_and_restart_preserve_dedupe_and_recover_incomplete_as_missing() {
    let directory = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("usage.db"))
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let completed = attribution("2026-10-31T23:59:59Z");
    let interrupted = attribution("2026-11-01T00:00:00Z");
    usage::accept(&pool, &completed).await.unwrap();
    usage::finish(
        &pool,
        completed.request_id,
        &UsageTokens::provider(Some(6), Some(4), Some(10)),
    )
    .await
    .unwrap();
    usage::accept(&pool, &interrupted).await.unwrap();
    // Raw retention is independent: even complete absence cannot remove usage or once-state.
    sqlx::query("DELETE FROM request_history")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let reopened = SqlitePool::connect_with(options).await.unwrap();
    assert_eq!(usage::recover_incomplete(&reopened).await.unwrap(), 1);
    assert_eq!(usage::recover_incomplete(&reopened).await.unwrap(), 0);
    usage::accept(&reopened, &completed).await.unwrap();
    usage::finish(
        &reopened,
        completed.request_id,
        &UsageTokens::provider(Some(6), Some(4), Some(10)),
    )
    .await
    .unwrap();
    usage::finish(
        &reopened,
        interrupted.request_id,
        &UsageTokens::provider(Some(100), Some(100), Some(200)),
    )
    .await
    .unwrap();
    let row = totals(&reopened).await;
    assert_eq!(row.get::<i64, _>("requests"), 2);
    assert_eq!(row.get::<i64, _>("total"), 10);
    assert_eq!(row.get::<i64, _>("missing"), 1);
    assert_eq!(row.get::<i64, _>("pending"), 0);
    let dates: Vec<String> = sqlx::query_scalar("SELECT date FROM usage_daily ORDER BY date")
        .fetch_all(&reopened)
        .await
        .unwrap();
    assert_eq!(dates, ["2026-10-31", "2026-11-01"]);
}

#[tokio::test]
async fn legacy_unattributed_and_deleted_identities_keep_separate_buckets_without_foreign_keys() {
    let pool = pool().await;
    let assigned = attribution("2026-11-01T00:00:00Z");
    let mut legacy = attribution("2026-11-01T00:00:00Z");
    legacy.account_id = None;
    legacy.organization_id = None;
    usage::accept(&pool, &legacy).await.unwrap();
    usage::accept(&pool, &assigned).await.unwrap();
    usage::finish(&pool, legacy.request_id, &UsageTokens::missing())
        .await
        .unwrap();
    usage::finish(&pool, assigned.request_id, &UsageTokens::missing())
        .await
        .unwrap();
    let buckets = sqlx::query("SELECT organization_id, account_id, request_count FROM usage_daily")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(buckets.len(), 2);
    assert!(buckets
        .iter()
        .any(|row| row.get::<String, _>("organization_id").is_empty()
            && row.get::<String, _>("account_id").is_empty()
            && row.get::<i64, _>("request_count") == 1));
    assert!(buckets
        .iter()
        .any(|row| row.get::<String, _>("organization_id")
            == assigned.organization_id.unwrap().to_string()
            && row.get::<String, _>("account_id") == assigned.account_id.unwrap().to_string()));
    let references = sqlx::query("PRAGMA foreign_key_list(usage_receipts)")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        references.is_empty(),
        "deleted users, keys, or organizations must not cascade-delete snapshots"
    );
}
