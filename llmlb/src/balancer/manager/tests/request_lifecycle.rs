use super::*;

#[tokio::test]
async fn request_lease_complete_releases_active_counter() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    let lease = load_manager
        .begin_request(endpoint_id)
        .await
        .expect("begin_request should succeed");

    let active_before = load_manager
        .snapshot(endpoint_id)
        .await
        .expect("snapshot should succeed")
        .active_requests;
    assert_eq!(active_before, 1);

    lease
        .complete(RequestOutcome::Success, StdDuration::from_millis(3))
        .await
        .expect("complete should succeed");

    let snapshot_after = load_manager
        .snapshot(endpoint_id)
        .await
        .expect("snapshot should succeed");
    assert_eq!(snapshot_after.active_requests, 0);
    assert_eq!(snapshot_after.successful_requests, 1);
}

#[tokio::test]
async fn request_lease_drop_auto_releases_active_counter() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;

    {
        let _lease = load_manager
            .begin_request(endpoint_id)
            .await
            .expect("begin_request should succeed");
    }

    let mut final_snapshot = None;
    for _ in 0..30 {
        let snapshot = load_manager
            .snapshot(endpoint_id)
            .await
            .expect("snapshot should succeed");
        if snapshot.active_requests == 0 {
            final_snapshot = Some(snapshot);
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }

    let snapshot = final_snapshot.expect("lease auto-complete should drain active requests");
    assert_eq!(snapshot.active_requests, 0);
    assert_eq!(snapshot.failed_requests, 1);
}

// ===== record_metrics テスト =====

#[tokio::test]
async fn record_metrics_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let update = MetricsUpdate {
        endpoint_id: Uuid::new_v4(),
        cpu_usage: 10.0,
        memory_usage: 20.0,
        gpu_usage: None,
        gpu_memory_usage: None,
        gpu_memory_total_mb: None,
        gpu_memory_used_mb: None,
        gpu_temperature: None,
        gpu_model_name: None,
        gpu_compute_capability: None,
        gpu_capability_score: None,
        active_requests: 0,
        average_response_time_ms: None,
        initializing: false,
        ready_models: None,
    };
    let result = load_manager.record_metrics(update).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn record_metrics_valid_endpoint_succeeds() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let update = MetricsUpdate {
        endpoint_id,
        cpu_usage: 50.0,
        memory_usage: 60.0,
        gpu_usage: Some(70.0),
        gpu_memory_usage: Some(80.0),
        gpu_memory_total_mb: Some(8192),
        gpu_memory_used_mb: Some(4096),
        gpu_temperature: Some(65.0),
        gpu_model_name: Some("NVIDIA RTX 4090".to_string()),
        gpu_compute_capability: Some("8.9".to_string()),
        gpu_capability_score: Some(90),
        active_requests: 2,
        average_response_time_ms: Some(150.0),
        initializing: false,
        ready_models: Some((1, 1)),
    };
    let result = load_manager.record_metrics(update).await;
    assert!(result.is_ok());

    let snapshot = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snapshot.cpu_usage, Some(50.0));
    assert_eq!(snapshot.memory_usage, Some(60.0));
    assert_eq!(snapshot.gpu_usage, Some(70.0));
}

// ===== begin_request / finish_request テスト =====

#[tokio::test]
async fn begin_request_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager.begin_request(Uuid::new_v4()).await;
    assert!(result.is_err());
}

// 選択(Online)→割当(begin_request)の間にエンドポイントが Offline/Error へ
// 遷移する TOCTOU を塞ぐ。begin_request は dead 状態への割当を拒否する。
#[tokio::test]
async fn begin_request_offline_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .endpoint_registry()
        .update_status(endpoint_id, EndpointStatus::Offline, None, Some("down"))
        .await
        .expect("update status to offline");
    let result = load_manager.begin_request(endpoint_id).await;
    assert!(
        result.is_err(),
        "begin_request must reject offline endpoints (TOCTOU guard)"
    );
}

#[tokio::test]
async fn begin_request_error_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    load_manager
        .endpoint_registry()
        .update_status(endpoint_id, EndpointStatus::Error, None, Some("boom"))
        .await
        .expect("update status to error");
    let result = load_manager.begin_request(endpoint_id).await;
    assert!(
        result.is_err(),
        "begin_request must reject error-state endpoints (TOCTOU guard)"
    );
}

// Pending は従来どおり許可する（begin_request は歴史的に状態を問わなかった。
// 本番では選択が Online のみ返すため、Pending への直接割当は回帰させない）。
#[tokio::test]
async fn begin_request_pending_endpoint_is_allowed() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    // setup の既定状態は Pending
    let result = load_manager.begin_request(endpoint_id).await;
    assert!(
        result.is_ok(),
        "begin_request must still allow pending endpoints"
    );
}

#[tokio::test]
async fn finish_request_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager
        .finish_request(
            Uuid::new_v4(),
            RequestOutcome::Success,
            StdDuration::from_millis(100),
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn finish_request_with_tokens_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager
        .finish_request_with_tokens(
            Uuid::new_v4(),
            RequestOutcome::Success,
            StdDuration::from_millis(100),
            None,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn finish_request_success_updates_counts() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();
    load_manager
        .finish_request(
            endpoint_id,
            RequestOutcome::Success,
            StdDuration::from_millis(100),
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snap.successful_requests, 1);
}

#[tokio::test]
async fn finish_request_error_updates_counts() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();
    load_manager
        .finish_request(
            endpoint_id,
            RequestOutcome::Error,
            StdDuration::from_millis(100),
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snap.failed_requests, 1);
}

#[tokio::test]
async fn finish_request_queued_does_not_decrement_active() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();
    load_manager
        .finish_request(
            endpoint_id,
            RequestOutcome::Queued,
            StdDuration::from_millis(0),
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    // Queued does not decrement active
    assert_eq!(snap.successful_requests, 0);
    assert_eq!(snap.failed_requests, 0);
}

// ===== finish_request_with_tokens テスト =====

#[tokio::test]
async fn finish_request_with_tokens_records_token_usage() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();

    let token_usage = Some(crate::token::TokenUsage {
        input_tokens: Some(100),
        output_tokens: Some(50),
        total_tokens: Some(150),
    });

    load_manager
        .finish_request_with_tokens(
            endpoint_id,
            RequestOutcome::Success,
            StdDuration::from_millis(200),
            token_usage,
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snap.total_input_tokens, 100);
    assert_eq!(snap.total_output_tokens, 50);
    assert_eq!(snap.total_tokens, 150);
}

#[tokio::test]
async fn finish_request_with_tokens_none_usage() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();

    load_manager
        .finish_request_with_tokens(
            endpoint_id,
            RequestOutcome::Success,
            StdDuration::from_millis(100),
            None,
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snap.total_input_tokens, 0);
    assert_eq!(snap.total_output_tokens, 0);
    assert_eq!(snap.total_tokens, 0);
}

#[tokio::test]
async fn finish_request_with_tokens_partial_usage() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let _lease = load_manager.begin_request(endpoint_id).await.unwrap();

    let token_usage = Some(crate::token::TokenUsage {
        input_tokens: Some(100),
        output_tokens: None,
        total_tokens: None,
    });

    load_manager
        .finish_request_with_tokens(
            endpoint_id,
            RequestOutcome::Success,
            StdDuration::from_millis(100),
            token_usage,
        )
        .await
        .unwrap();

    let snap = load_manager.snapshot(endpoint_id).await.unwrap();
    assert_eq!(snap.total_input_tokens, 100);
    assert_eq!(snap.total_output_tokens, 0);
    // total_tokens is derived: input(100) + output(None) = Some(100)
    assert_eq!(snap.total_tokens, 100);
}
