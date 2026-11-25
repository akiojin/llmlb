//! Issue #981: 異常終了ストリームの成否計上と SSE 行バッファ上限のテスト
//!
//! 中断されたストリームを成功として計上しないことと、行バッファの上限で
//! 破棄しても後続の行を処理できることの検証。proxy.rs の行数上限
//! （FR-009）のため tests 配下へ分離した。

use super::*;

// --- Issue #981: 異常終了ストリームは成功として計上しない ---

async fn setup_stats_test_registry() -> (
    crate::registry::endpoints::EndpointRegistry,
    crate::balancer::LoadManager,
    crate::events::SharedEventBus,
    uuid::Uuid,
) {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("create test db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    let registry = crate::registry::endpoints::EndpointRegistry::new(pool)
        .await
        .expect("create registry");
    let endpoint = Endpoint::new(
        "drop-stats-ep".to_string(),
        "http://localhost:1".to_string(),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    registry.add(endpoint).await.expect("add endpoint");
    let load_manager = crate::balancer::LoadManager::new(Arc::new(registry.clone()));
    let event_bus = crate::events::create_shared_event_bus();
    (registry, load_manager, event_bus, endpoint_id)
}

async fn wait_for_recorded_stats(
    registry: &crate::registry::endpoints::EndpointRegistry,
    endpoint_id: uuid::Uuid,
) -> Endpoint {
    for _ in 0..200 {
        let endpoint = registry.get(endpoint_id).await.expect("endpoint exists");
        if endpoint.total_requests >= 1 {
            return endpoint;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    registry.get(endpoint_id).await.expect("endpoint exists")
}

#[tokio::test]
async fn streaming_tracker_drop_records_failure_not_success() {
    let (registry, load_manager, event_bus, endpoint_id) = setup_stats_test_registry().await;

    let response = reqwest::Response::from(
        axum::http::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n")
            .unwrap(),
    );

    let axum_response = forward_streaming_response_with_tps_tracking(
        response,
        endpoint_id,
        "m".to_string(),
        Some(TpsApiKind::ChatCompletions),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
        Instant::now(),
        registry.clone(),
        load_manager,
        event_bus,
        None,
        STREAM_IDLE_TIMEOUT,
    )
    .expect("forward streaming");

    // クライアント切断を模し、ボディを消費せずに drop する
    drop(axum_response);

    let endpoint = wait_for_recorded_stats(&registry, endpoint_id).await;
    assert_eq!(endpoint.total_requests, 1);
    assert_eq!(
        endpoint.successful_requests, 0,
        "aborted stream must not be recorded as success"
    );
    assert_eq!(endpoint.failed_requests, 1);
}

#[tokio::test]
async fn streaming_tracker_completion_records_success() {
    let (registry, load_manager, event_bus, endpoint_id) = setup_stats_test_registry().await;

    let response = reqwest::Response::from(
        axum::http::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n")
            .unwrap(),
    );

    let axum_response = forward_streaming_response_with_tps_tracking(
        response,
        endpoint_id,
        "m".to_string(),
        Some(TpsApiKind::ChatCompletions),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
        Instant::now(),
        registry.clone(),
        load_manager,
        event_bus,
        None,
        STREAM_IDLE_TIMEOUT,
    )
    .expect("forward streaming");

    let _ = axum::body::to_bytes(axum_response.into_body(), usize::MAX)
        .await
        .expect("consume body");

    let endpoint = wait_for_recorded_stats(&registry, endpoint_id).await;
    assert_eq!(endpoint.total_requests, 1);
    assert_eq!(endpoint.successful_requests, 1);
    assert_eq!(endpoint.failed_requests, 0);
}

// --- Issue #981: SSE行バッファの上限 ---

#[test]
fn process_sse_lines_discards_oversized_partial_line() {
    let mut buffer = String::new();
    let mut acc = StreamingTokenAccumulator::new("test-model");
    // 改行なしで上限（1MiB）を超える部分行はバッファに残り続けない
    let oversized = "x".repeat(2 * 1024 * 1024);
    process_sse_lines(&mut buffer, &oversized, &mut acc);
    assert!(
        buffer.len() <= 1024 * 1024,
        "oversized partial line must be discarded, buffer len = {}",
        buffer.len()
    );
}

#[test]
fn process_sse_lines_buffer_stays_bounded_across_chunks() {
    let mut buffer = String::new();
    let mut acc = StreamingTokenAccumulator::new("test-model");
    // 改行なしチャンクが続いてもバッファは上限以下に保たれる
    let chunk = "y".repeat(64 * 1024);
    for _ in 0..64 {
        process_sse_lines(&mut buffer, &chunk, &mut acc);
        assert!(
            buffer.len() <= 1024 * 1024,
            "buffer must stay bounded, len = {}",
            buffer.len()
        );
    }
}

#[test]
fn process_sse_lines_long_line_under_cap_still_accumulates() {
    let mut buffer = String::new();
    let mut acc = StreamingTokenAccumulator::new("test-model");
    // 上限未満の長い行はチャンクをまたいで蓄積され、改行で処理される
    let half = "z".repeat(256 * 1024);
    process_sse_lines(&mut buffer, &half, &mut acc);
    assert_eq!(buffer.len(), 256 * 1024);
    process_sse_lines(&mut buffer, &half, &mut acc);
    assert_eq!(buffer.len(), 512 * 1024);
    process_sse_lines(&mut buffer, "\n", &mut acc);
    assert!(buffer.is_empty());
}

#[test]
fn process_sse_lines_recovers_after_oversized_discard() {
    let mut buffer = String::new();
    let mut acc = StreamingTokenAccumulator::new("test-model");
    let oversized = "x".repeat(2 * 1024 * 1024);
    process_sse_lines(&mut buffer, &oversized, &mut acc);
    // 上限超過で破棄された後でも、後続の通常の行は処理できる
    process_sse_lines(&mut buffer, "data: line1\n", &mut acc);
    assert!(buffer.is_empty());
}
