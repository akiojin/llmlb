//! Ordering regressions for the body-free ledger.

use super::*;
use uuid::Uuid;

async fn pool() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE request_history(id TEXT PRIMARY KEY, timestamp TEXT); CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL);")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("../../../migrations/20261030050100_usage.sql"))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn usage_late_initial_cannot_move_terminal_target_or_replace_reported_tokens() {
    let pool = pool().await;
    let attribution = UsageAttribution {
        request_id: Uuid::new_v4(),
        received_at: chrono::Utc::now(),
        organization_id: Some(Uuid::new_v4()),
        account_id: Some(Uuid::new_v4()),
        api_key_id: None,
        auth_source: "jwt".into(),
    };
    let final_tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
        TokenSource::Provider,
    );
    let old_tokens = UsageTokens::from_usage(
        &crate::token::TokenUsage::new(Some(100), Some(100), Some(200)),
        TokenSource::Provider,
    );
    let mut transaction = pool.begin().await.unwrap();
    save_in_transaction(
        &mut transaction,
        &attribution,
        "final-endpoint",
        "final-model",
        &final_tokens,
        true,
        true,
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let mut transaction = pool.begin().await.unwrap();
    save_in_transaction(
        &mut transaction,
        &attribution,
        "older-attempt",
        "older-model",
        &old_tokens,
        false,
        false,
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();
    let stored: (String, String, bool, bool, i64) = sqlx::query_as(
        "SELECT endpoint_id, model, finalized, successful, total_tokens FROM usage_receipts",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            "final-endpoint".into(),
            "final-model".into(),
            true,
            true,
            18
        )
    );
    let totals: (i64, i64, i64) = sqlx::query_as(
        "SELECT SUM(request_count), SUM(total_tokens), SUM(successful_requests) FROM usage_daily",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(totals, (1, 18, 1));
}
