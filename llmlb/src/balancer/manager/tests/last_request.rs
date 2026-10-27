use super::*;

fn request_time(endpoint: &Endpoint) -> Option<chrono::DateTime<Utc>> {
    let json = serde_json::to_value(endpoint).unwrap();
    assert!(
        json.get("last_request_at").is_some(),
        "nullable last_request_at contract"
    );
    json["last_request_at"].as_str().map(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    })
}

#[tokio::test]
async fn last_request_is_durable_before_long_running_request_completes() {
    let dir = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(dir.path().join("last-request.db"))
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options.clone()).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let registry = Arc::new(EndpointRegistry::new(pool.clone()).await.unwrap());
    let endpoint = Endpoint::new(
        "durable-start".into(),
        "http://localhost:9000".into(),
        EndpointType::Xllm,
    );
    let id = endpoint.id;
    registry.add(endpoint).await.unwrap();
    assert!(request_time(&registry.get(id).await.unwrap()).is_none());
    let manager = LoadManager::new(registry.clone());
    let before = Utc::now();
    let lease = manager.begin_request(id).await.unwrap();
    let started = request_time(&registry.get(id).await.unwrap()).unwrap();
    assert!(started >= before && started <= Utc::now());
    assert!(registry.get(id).await.unwrap().last_seen.is_none());

    // Reopen the database while the lease is still outstanding (including streams).
    let reopened_pool = SqlitePool::connect_with(options).await.unwrap();
    let reopened = EndpointRegistry::new(reopened_pool).await.unwrap();
    assert_eq!(
        request_time(&reopened.get(id).await.unwrap()),
        Some(started)
    );
    lease
        .complete(RequestOutcome::Success, StdDuration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(
        request_time(&registry.get(id).await.unwrap()),
        Some(started)
    );
}

#[tokio::test]
async fn last_request_health_completion_and_stale_edit_do_not_reorder() {
    let (manager, id) = setup_test_load_manager().await;
    let registry = &manager.endpoint_registry;
    let stale = registry.get(id).await.unwrap();
    let old = manager.begin_request(id).await.unwrap();
    let newer = manager.begin_request(id).await.unwrap();
    let started = request_time(&registry.get(id).await.unwrap()).unwrap();
    registry
        .update_status(id, EndpointStatus::Online, Some(5), None)
        .await
        .unwrap();
    old.complete(RequestOutcome::Error, StdDuration::from_secs(90))
        .await
        .unwrap();
    assert_eq!(
        request_time(&registry.get(id).await.unwrap()),
        Some(started)
    );
    registry.update(stale).await.unwrap();
    assert_eq!(
        request_time(&registry.get(id).await.unwrap()),
        Some(started)
    );
    newer
        .complete(RequestOutcome::Success, StdDuration::ZERO)
        .await
        .unwrap();
    assert_eq!(
        request_time(&registry.get(id).await.unwrap()),
        Some(started)
    );
}

#[tokio::test]
async fn last_request_rejected_assignment_and_health_leave_null() {
    let (manager, id) = setup_test_load_manager().await;
    manager
        .endpoint_registry
        .update_status(id, EndpointStatus::Offline, None, None)
        .await
        .unwrap();
    assert!(manager.begin_request(id).await.is_err());
    assert!(manager.begin_request(Uuid::new_v4()).await.is_err());
    assert!(request_time(&manager.endpoint_registry.get(id).await.unwrap()).is_none());
}

#[tokio::test]
async fn last_request_cancelled_lease_keeps_assignment_timestamp() {
    let (manager, id) = setup_test_load_manager().await;
    let lease = manager.begin_request(id).await.unwrap();
    let started = request_time(&manager.endpoint_registry.get(id).await.unwrap()).unwrap();
    drop(lease);
    tokio::time::timeout(StdDuration::from_secs(2), async {
        while manager.snapshot(id).await.unwrap().active_requests != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        request_time(&manager.endpoint_registry.get(id).await.unwrap()),
        Some(started)
    );
}

#[tokio::test]
async fn last_request_storage_failure_rejects_assignment_without_activity() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let registry = Arc::new(EndpointRegistry::new(pool.clone()).await.unwrap());
    let endpoint = Endpoint::new(
        "closed-storage".into(),
        "http://localhost:9000".into(),
        EndpointType::Xllm,
    );
    let id = endpoint.id;
    registry.add(endpoint).await.unwrap();
    let manager = LoadManager::new(registry.clone());
    pool.close().await;
    assert!(matches!(
        manager.begin_request(id).await,
        Err(LbError::Database(_))
    ));
    assert!(request_time(&registry.get(id).await.unwrap()).is_none());
    assert_eq!(manager.snapshot(id).await.unwrap().active_requests, 0);
}
