use super::*;

// ===== Issue #975: リクエスト失敗のステータス反映テスト =====

async fn add_online_endpoint(registry: &EndpointRegistry) -> Uuid {
    let mut endpoint = Endpoint::new(
        "Request Feedback".to_string(),
        "http://localhost:11434".to_string(),
        EndpointType::Xllm,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    registry.add(endpoint).await.unwrap();
    endpoint_id
}

#[tokio::test]
async fn test_record_request_failure_moves_online_endpoint_to_error() {
    let _lock = TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let registry = EndpointRegistry::new(pool).await.unwrap();
    let endpoint_id = add_online_endpoint(&registry).await;

    let transitioned = registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    assert!(transitioned);

    let updated = registry.get(endpoint_id).await.unwrap();
    assert_eq!(updated.status, EndpointStatus::Error);
    assert_eq!(updated.last_error.as_deref(), Some("inference timeout"));
    assert_eq!(updated.error_count, 1);
}

#[tokio::test]
async fn test_record_request_failure_twice_moves_endpoint_offline() {
    let _lock = TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let registry = EndpointRegistry::new(pool).await.unwrap();
    let endpoint_id = add_online_endpoint(&registry).await;

    registry
        .record_request_failure(endpoint_id, "first failure")
        .await
        .unwrap();
    // Error状態のまま2回目の失敗でOfflineへ遷移する
    let transitioned = registry
        .record_request_failure(endpoint_id, "second failure")
        .await
        .unwrap();
    assert!(!transitioned);

    let updated = registry.get(endpoint_id).await.unwrap();
    assert_eq!(updated.status, EndpointStatus::Offline);
}

#[tokio::test]
async fn test_record_request_failure_streak_survives_health_recovery() {
    let _lock = TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let registry = EndpointRegistry::new(pool).await.unwrap();
    let endpoint_id = add_online_endpoint(&registry).await;

    // 1回目の失敗でError
    registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    // ヘルスチェック成功でOnlineへ復帰しても、リクエスト失敗の連続カウントは残る
    registry
        .update_status(endpoint_id, EndpointStatus::Online, None, None)
        .await
        .unwrap();
    // 2回目のリクエスト失敗でOffline（/v1/modelsが成功し続けても推論の詰まりを検知できる）
    let transitioned = registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    assert!(transitioned);
    assert_eq!(
        registry.get(endpoint_id).await.unwrap().status,
        EndpointStatus::Offline
    );
}

#[tokio::test]
async fn test_record_request_success_resets_failure_streak() {
    let _lock = TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let registry = EndpointRegistry::new(pool).await.unwrap();
    let endpoint_id = add_online_endpoint(&registry).await;

    registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    registry
        .update_status(endpoint_id, EndpointStatus::Online, None, None)
        .await
        .unwrap();
    // 成功リクエストが挟まれば連続カウントはリセットされる
    registry.record_request_success(endpoint_id).await;

    registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    assert_eq!(
        registry.get(endpoint_id).await.unwrap().status,
        EndpointStatus::Error
    );
}

#[tokio::test]
async fn test_record_request_failure_ignores_offline_and_unknown_endpoints() {
    let _lock = TEST_LOCK.lock().await;
    let pool = setup_test_db().await;
    let registry = EndpointRegistry::new(pool).await.unwrap();
    let endpoint_id = add_online_endpoint(&registry).await;

    registry
        .update_status(endpoint_id, EndpointStatus::Offline, None, None)
        .await
        .unwrap();
    let transitioned = registry
        .record_request_failure(endpoint_id, "inference timeout")
        .await
        .unwrap();
    assert!(!transitioned);
    assert_eq!(
        registry.get(endpoint_id).await.unwrap().status,
        EndpointStatus::Offline
    );

    // 存在しないエンドポイントはエラーにせず無視する
    let transitioned = registry
        .record_request_failure(Uuid::new_v4(), "inference timeout")
        .await
        .unwrap();
    assert!(!transitioned);
}
