use super::*;

#[tokio::test]
async fn load_manager_cache_key_is_stable_and_unique_per_instance() {
    let _lock = TEST_LOCK.lock().await;

    let (load_manager, _) = setup_test_load_manager().await;
    let cloned = load_manager.clone();
    assert_eq!(load_manager.cache_key(), cloned.cache_key());

    let (another, _) = setup_test_load_manager().await;
    assert_ne!(load_manager.cache_key(), another.cache_key());
}

// ===== endpoint_registry テスト =====

#[tokio::test]
async fn endpoint_registry_returns_registry() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let registry = load_manager.endpoint_registry();
    let ep = registry.get(endpoint_id).await;
    assert!(ep.is_some());
}

// ===== snapshot / snapshots テスト =====

#[tokio::test]
async fn snapshot_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager.snapshot(Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn snapshots_returns_all_endpoints() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let snaps = load_manager.snapshots().await;
    assert_eq!(snaps.len(), 1);
}

// ===== summary テスト =====

#[tokio::test]
async fn summary_empty_state() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let summary = load_manager.summary().await;
    assert_eq!(summary.total_nodes, 1); // setup creates 1 endpoint
    assert_eq!(summary.total_requests, 0);
    assert_eq!(summary.successful_requests, 0);
    assert_eq!(summary.failed_requests, 0);
}
