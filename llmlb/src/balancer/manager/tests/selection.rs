use super::*;

// SPEC-f8e3a1b7: NodeRegistry依存のテストは削除されました
// - load_manager_prefers_lower_latency_when_active_equal
// - metrics_history_tracks_recent_points
// - wait_for_ready_unblocks_when_node_becomes_ready
// - wait_for_ready_limits_waiters_and_notifies_first
// 新しいEndpointRegistryベースのテストは tests/integration/ に追加してください

// T004: WaitResult enum テスト
#[test]
fn wait_result_enum_variants_exist() {
    // WaitResultの3つのバリアントが存在することを確認
    let ready = WaitResult::Ready;
    let timeout = WaitResult::Timeout;
    let capacity_exceeded = WaitResult::CapacityExceeded;

    // PartialEq実装の確認
    assert_eq!(ready, WaitResult::Ready);
    assert_eq!(timeout, WaitResult::Timeout);
    assert_eq!(capacity_exceeded, WaitResult::CapacityExceeded);
    assert_ne!(ready, timeout);

    // Debug実装の確認
    assert!(!format!("{:?}", ready).is_empty());
}

#[tokio::test]
async fn select_endpoint_round_robin_ready_for_model_excludes_initializing_endpoints() {
    let _lock = TEST_LOCK.lock().await;
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let registry = EndpointRegistry::new(pool)
        .await
        .expect("Failed to create endpoint registry");

    let mut ready_endpoint = Endpoint::new(
        "ready-endpoint".to_string(),
        "http://localhost:11080".to_string(),
        EndpointType::OpenaiCompatible,
    );
    ready_endpoint.status = EndpointStatus::Online;
    let ready_endpoint_id = ready_endpoint.id;
    registry
        .add(ready_endpoint)
        .await
        .expect("Failed to add ready endpoint");

    let mut initializing_endpoint = Endpoint::new(
        "initializing-endpoint".to_string(),
        "http://localhost:11081".to_string(),
        EndpointType::OpenaiCompatible,
    );
    initializing_endpoint.status = EndpointStatus::Online;
    let initializing_endpoint_id = initializing_endpoint.id;
    registry
        .add(initializing_endpoint)
        .await
        .expect("Failed to add initializing endpoint");

    let model_id = "gpt-oss:latest".to_string();
    for endpoint_id in [ready_endpoint_id, initializing_endpoint_id] {
        registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: model_id.clone(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
                canonical_name: None,
            })
            .await
            .expect("Failed to add endpoint model");
    }

    let load_manager = LoadManager::new(Arc::new(registry));
    load_manager
        .upsert_initial_state(ready_endpoint_id, false, Some((1, 1)))
        .await;
    load_manager
        .upsert_initial_state(initializing_endpoint_id, true, Some((0, 1)))
        .await;

    for _ in 0..4 {
        let selected = load_manager
            .select_endpoint_round_robin_ready_for_model(&model_id)
            .await
            .expect("selection should succeed");
        assert_eq!(
            selected.id, ready_endpoint_id,
            "initializing endpoint must not be selected"
        );
    }
}

// ===== compute_round_robin_priority_for_endpoints テスト =====

#[test]
fn compute_round_robin_priority_empty() {
    let endpoints: Vec<crate::types::endpoint::Endpoint> = vec![];
    let priority = super::compute_round_robin_priority_for_endpoints(&endpoints, 0);
    assert!(priority.is_empty());
}

#[test]
fn compute_round_robin_priority_single() {
    let ep = Endpoint::new(
        "ep1".to_string(),
        "http://localhost:1".to_string(),
        EndpointType::OpenaiCompatible,
    );
    let id = ep.id;
    let endpoints = vec![ep];
    let priority = super::compute_round_robin_priority_for_endpoints(&endpoints, 0);
    assert_eq!(priority.len(), 1);
    assert_eq!(priority[&id], 0);
}

#[test]
fn compute_round_robin_priority_wraps_around() {
    let ep1 = Endpoint::new(
        "ep1".to_string(),
        "http://localhost:1".to_string(),
        EndpointType::OpenaiCompatible,
    );
    let ep2 = Endpoint::new(
        "ep2".to_string(),
        "http://localhost:2".to_string(),
        EndpointType::OpenaiCompatible,
    );
    let ep3 = Endpoint::new(
        "ep3".to_string(),
        "http://localhost:3".to_string(),
        EndpointType::OpenaiCompatible,
    );
    let id1 = ep1.id;
    let id2 = ep2.id;
    let id3 = ep3.id;
    let endpoints = vec![ep1, ep2, ep3];
    // start_index=1 => priority: ep2=0, ep3=1, ep1=2
    let priority = super::compute_round_robin_priority_for_endpoints(&endpoints, 1);
    assert_eq!(priority[&id2], 0);
    assert_eq!(priority[&id3], 1);
    assert_eq!(priority[&id1], 2);
}

// ===== has_ready_nodes / all_initializing テスト =====

#[tokio::test]
async fn has_ready_nodes_empty_state() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    // 初期状態ではstateにエントリなし
    assert!(!load_manager.has_ready_nodes().await);
}

#[tokio::test]
async fn has_ready_nodes_with_ready_endpoint() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .upsert_initial_state(endpoint_id, false, Some((1, 1)))
        .await;
    assert!(load_manager.has_ready_nodes().await);
}

#[tokio::test]
async fn has_ready_nodes_all_initializing() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .upsert_initial_state(endpoint_id, true, Some((0, 1)))
        .await;
    assert!(!load_manager.has_ready_nodes().await);
}

#[tokio::test]
async fn all_initializing_empty() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    // stateが空の場合はfalse
    assert!(!load_manager.all_initializing().await);
}

#[tokio::test]
async fn all_initializing_true_when_all_are() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .upsert_initial_state(endpoint_id, true, Some((0, 1)))
        .await;
    assert!(load_manager.all_initializing().await);
}

#[tokio::test]
async fn all_initializing_false_when_one_ready() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .upsert_initial_state(endpoint_id, false, Some((1, 1)))
        .await;
    assert!(!load_manager.all_initializing().await);
}

// ===== queue_waiters テスト =====

#[tokio::test]
async fn queue_waiters_starts_at_zero() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    assert_eq!(load_manager.queue_waiters(), 0);
}

// ===== select_endpoint_direct テスト =====

#[tokio::test]
async fn select_endpoint_direct_no_online_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    let registry = EndpointRegistry::new(pool)
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(registry));

    let result = load_manager.select_endpoint_direct().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn select_endpoint_direct_selects_online() {
    let _lock = TEST_LOCK.lock().await;
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    let registry = EndpointRegistry::new(pool)
        .await
        .expect("Failed to create endpoint registry");
    let mut ep = Endpoint::new(
        "online-ep".to_string(),
        "http://localhost:11434".to_string(),
        EndpointType::OpenaiCompatible,
    );
    ep.status = EndpointStatus::Online;
    registry.add(ep).await.expect("Failed to add endpoint");

    let load_manager = LoadManager::new(Arc::new(registry));
    let result = load_manager.select_endpoint_direct().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().name, "online-ep");
}

// ===== select_endpoint_direct_for_model テスト =====

#[tokio::test]
async fn select_endpoint_direct_for_model_no_match() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager
        .select_endpoint_direct_for_model("nonexistent-model")
        .await;
    assert!(result.is_err());
}

// ===== wait_for_idle_node_with_timeout テスト =====

#[tokio::test]
async fn wait_for_idle_node_capacity_exceeded() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager
        .wait_for_idle_node_with_timeout(0, StdDuration::from_millis(100))
        .await;
    assert_eq!(result, WaitResult::CapacityExceeded);
}

// ===== wait_for_idle_node_with_timeout_for_model テスト =====

#[tokio::test]
async fn wait_for_idle_node_for_model_capacity_exceeded() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager
        .wait_for_idle_node_with_timeout_for_model("test-model", 0, StdDuration::from_millis(100))
        .await;
    assert_eq!(result, WaitResult::CapacityExceeded);
}
