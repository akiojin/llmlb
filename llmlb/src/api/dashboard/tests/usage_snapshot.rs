//! A usage page must use one read snapshot while terminal writes commit.

use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn usage_page_keeps_totals_groups_and_rows_in_one_snapshot() {
    let releases = Arc::new(AtomicUsize::new(0));
    let callback_releases = releases.clone();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .after_release(move |connection, _| {
            let releases = callback_releases.clone();
            Box::pin(async move {
                if releases
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    == Ok(1)
                {
                    // Settle the admitted request and accept another account's
                    // request exactly after the first reader releases its lease.
                    sqlx::query("UPDATE usage_daily SET input_tokens=33,output_tokens=21,total_tokens=54,provider_count=3,missing_count=0")
                        .execute(&mut *connection).await?;
                    sqlx::query("INSERT INTO usage_daily (date,account_id,request_count,input_tokens,output_tokens,total_tokens,provider_count) VALUES (DATE('now'),'new-account',1,11,7,18,1)")
                        .execute(&mut *connection).await?;
                }
                Ok(true)
            })
        })
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("INSERT INTO usage_daily (date,account_id,request_count,input_tokens,output_tokens,total_tokens,provider_count,missing_count) VALUES (DATE('now'),'existing-account',3,22,14,36,2,1)")
        .execute(&pool).await.unwrap();
    let mut state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    state.db_pool = pool.clone();

    // Drain setup releases before arming the interleaving. This guard's release
    // consumes the first count; the handler's first connection release settles.
    let setup_guard = pool.acquire().await.unwrap();
    releases.store(2, Ordering::SeqCst);
    drop(setup_guard);
    let Json(page) = super::super::usage::get_usage(
        State(state),
        Extension(UsageScope::Global),
        axum::extract::Query(super::super::usage::UsageQuery::default()),
    )
    .await
    .unwrap();

    assert_eq!(
        page.totals.total_tokens,
        page.rows.iter().map(|r| r.counts.total_tokens).sum::<i64>()
    );
    assert_eq!(
        page.totals.measured_requests,
        page.rows
            .iter()
            .map(|r| r.counts.measured_requests)
            .sum::<i64>()
    );
    assert_eq!(
        page.totals.missing_requests,
        page.rows
            .iter()
            .map(|r| r.counts.missing_requests)
            .sum::<i64>()
    );
    assert_eq!(page.total_count, page.rows.len() as i64);
    assert_eq!(
        page.rows.len(),
        1,
        "the page reflects the pre-settlement snapshot"
    );
    let settled: i64 = sqlx::query_scalar("SELECT SUM(total_tokens) FROM usage_daily")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(settled, 72, "the concurrent settlement did execute");
}
