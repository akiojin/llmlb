use super::*;

#[tokio::test]
async fn last_request_caller_cancellation_keeps_database_and_cache_consistent() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let registry = EndpointRegistry::new(pool.clone()).await.unwrap();
    let endpoint = Endpoint::new(
        "cancelled-start".into(),
        "http://localhost:9000".into(),
        EndpointType::Xllm,
    );
    let id = endpoint.id;
    registry.add(endpoint).await.unwrap();
    let started = chrono::Utc::now();

    // Pause cache publication after SQLite has accepted the assignment.
    let cache = registry.endpoints.write().await;
    let caller = tokio::spawn({
        let registry = registry.clone();
        async move { registry.record_request_started(id, started).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if db::get_endpoint(&pool, id)
                .await
                .unwrap()
                .unwrap()
                .last_request_at
                == Some(started)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("start persisted before caller cancellation");
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    drop(cache);

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while registry.get(id).await.unwrap().last_request_at != Some(started) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled caller must not leave the dashboard cache stale");
}
