use super::*;
use crate::db::test_utils::TestAppStateBuilder;
use crate::events::DashboardEvent;

#[test]
fn activity_ring_counts_starts_by_second_and_zero_fills_gaps() {
    use crate::balancer::activity::RequestRateRing;
    let mut ring = RequestRateRing::default();
    assert_eq!(ring.series(100), vec![0; 60]);
    ring.record(100);
    ring.record(100);
    ring.record(102);
    let series = ring.series(102);
    assert_eq!(series.len(), 60);
    assert!(series[..57].iter().all(|&count| count == 0));
    assert_eq!(&series[57..], &[2, 0, 1]);
}

#[test]
fn activity_ring_expires_at_sixty_seconds_and_reuses_slots() {
    use crate::balancer::activity::RequestRateRing;
    let mut ring = RequestRateRing::default();
    ring.record(100);
    assert_eq!(ring.series(159)[0], 1, "59-second-old start is retained");
    assert_eq!(ring.series(160), vec![0; 60], "60-second-old start expires");
    ring.record(160);
    assert_eq!(
        ring.series(160)[59],
        1,
        "slot reuse must not retain old count"
    );
    assert_eq!(
        ring.series(10_000),
        vec![0; 60],
        "idle windows contain zeros"
    );
}

#[test]
fn activity_history_tracks_global_and_endpoint_series_and_forgets_deleted_endpoints() {
    use crate::balancer::activity::RequestActivityHistory;
    let mut history = RequestActivityHistory::default();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    history.record(first, 100);
    history.record(second, 100);
    history.record(first, 101);
    assert_eq!(&history.total_series(101)[58..], &[2, 1]);
    assert_eq!(&history.endpoint_series(first, 101)[58..], &[1, 1]);
    assert_eq!(&history.endpoint_series(second, 101)[58..], &[1, 0]);
    history.forget_endpoint(first);
    assert_eq!(history.endpoint_series(first, 101), vec![0; 60]);
    assert_eq!(
        &history.total_series(101)[58..],
        &[2, 1],
        "global flow survives deletion"
    );
}

#[tokio::test]
async fn activity_snapshot_keeps_global_rate_when_endpoint_is_deleted() {
    let _lock = TEST_LOCK.lock().await;
    let (manager, endpoint_id) = setup_test_load_manager().await;
    let lease = manager.begin_request(endpoint_id).await.unwrap();
    let activity = manager.request_activity().await;
    assert_eq!(activity.total.active_requests, 1);
    assert_eq!(
        activity.endpoints[0]
            .requests_per_second
            .iter()
            .sum::<u64>(),
        1
    );
    lease
        .complete(RequestOutcome::Success, StdDuration::from_millis(1))
        .await
        .unwrap();
    manager.endpoint_registry.remove(endpoint_id).await.unwrap();
    manager.forget_endpoint(endpoint_id).await;
    let activity = manager.request_activity().await;
    assert!(activity.endpoints.is_empty());
    assert_eq!(activity.total.active_requests, 0);
    assert_eq!(activity.total.requests_per_second.iter().sum::<u64>(), 1);
}

#[tokio::test]
async fn activity_snapshot_does_not_count_rejected_starts_or_completions_as_arrivals() {
    let _lock = TEST_LOCK.lock().await;
    let (manager, endpoint_id) = setup_test_load_manager().await;
    assert!(manager.begin_request(Uuid::new_v4()).await.is_err());
    assert_eq!(
        manager
            .request_activity()
            .await
            .total
            .requests_per_second
            .iter()
            .sum::<u64>(),
        0
    );
    let lease = manager.begin_request(endpoint_id).await.unwrap();
    lease
        .complete(RequestOutcome::Error, StdDuration::from_millis(1))
        .await
        .unwrap();
    let activity = manager.request_activity().await;
    assert_eq!(activity.total.active_requests, 0);
    assert_eq!(activity.total.requests_per_second.iter().sum::<u64>(), 1);
    assert_eq!(
        activity.endpoints[0]
            .requests_per_second
            .iter()
            .sum::<u64>(),
        1
    );
}

async fn activity_test_state() -> (crate::AppState, Uuid) {
    let state = TestAppStateBuilder::new().await.build().await;
    let endpoint = Endpoint::new(
        "activity-test".into(),
        "http://localhost:11434".into(),
        EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    (state, endpoint_id)
}

async fn next_activity_notification(
    receiver: &mut tokio::sync::broadcast::Receiver<DashboardEvent>,
    endpoint_id: Uuid,
) {
    let event = tokio::time::timeout(StdDuration::from_millis(1750), receiver.recv())
        .await
        .expect("activity changes must be notified within two seconds")
        .expect("event bus remains open");
    assert!(matches!(
        event,
        DashboardEvent::MetricsUpdated {
            runtime_id,
            cpu_usage: None,
            memory_usage: None,
            gpu_usage: None,
        } if runtime_id == endpoint_id
    ));
    assert!(
        receiver.try_recv().is_err(),
        "one event per endpoint/window"
    );
}

#[tokio::test]
async fn activity_start_and_success_completion_use_existing_coalesced_metrics() {
    let _lock = TEST_LOCK.lock().await;
    let (state, endpoint_id) = activity_test_state().await;
    tokio::time::pause();
    let manager = &state.balancer.load_manager;
    let mut receiver = state.event_bus.subscribe();
    // SQLite runs on an external worker. Keep the runtime runnable while starts
    // await storage, so paused Tokio time cannot auto-advance the coalescing timer.
    let keep_clock_paused = tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    });
    let first = manager.begin_request(endpoint_id).await.unwrap();
    let second = manager.begin_request(endpoint_id).await.unwrap();
    assert!(
        receiver.try_recv().is_err(),
        "the one-second window is retained"
    );
    keep_clock_paused.abort();
    next_activity_notification(&mut receiver, endpoint_id).await;
    assert_eq!(
        manager.snapshot(endpoint_id).await.unwrap().active_requests,
        2
    );

    first
        .complete(RequestOutcome::Success, StdDuration::from_secs(40))
        .await
        .unwrap();
    second
        .complete(RequestOutcome::Success, StdDuration::from_secs(40))
        .await
        .unwrap();
    next_activity_notification(&mut receiver, endpoint_id).await;
    assert_eq!(
        manager.snapshot(endpoint_id).await.unwrap().active_requests,
        0
    );
}

#[tokio::test]
async fn activity_error_with_tokens_completion_notifies_and_releases_counter() {
    let _lock = TEST_LOCK.lock().await;
    let (state, endpoint_id) = activity_test_state().await;
    tokio::time::pause();
    let manager = &state.balancer.load_manager;
    let mut receiver = state.event_bus.subscribe();
    let lease = manager.begin_request(endpoint_id).await.unwrap();
    next_activity_notification(&mut receiver, endpoint_id).await;
    lease
        .complete_with_tokens(RequestOutcome::Error, StdDuration::from_millis(3), None)
        .await
        .unwrap();
    next_activity_notification(&mut receiver, endpoint_id).await;
    assert_eq!(
        manager.snapshot(endpoint_id).await.unwrap().active_requests,
        0
    );
}

#[tokio::test]
async fn activity_dropped_lease_notifies_and_releases_counter() {
    let _lock = TEST_LOCK.lock().await;
    let (state, endpoint_id) = activity_test_state().await;
    tokio::time::pause();
    let manager = &state.balancer.load_manager;
    let mut receiver = state.event_bus.subscribe();
    let lease = manager.begin_request(endpoint_id).await.unwrap();
    next_activity_notification(&mut receiver, endpoint_id).await;
    drop(lease);
    next_activity_notification(&mut receiver, endpoint_id).await;
    assert_eq!(
        manager.snapshot(endpoint_id).await.unwrap().active_requests,
        0
    );
}

#[tokio::test]
async fn activity_rejected_start_does_not_emit_a_notification() {
    let _lock = TEST_LOCK.lock().await;
    let (state, endpoint_id) = activity_test_state().await;
    let mut receiver = state.event_bus.subscribe();
    state
        .balancer
        .endpoint_registry
        .update_status(endpoint_id, EndpointStatus::Offline, None, None)
        .await
        .unwrap();
    tokio::time::pause();
    assert!(state
        .balancer
        .load_manager
        .begin_request(endpoint_id)
        .await
        .is_err());
    assert!(state
        .balancer
        .load_manager
        .begin_request(Uuid::new_v4())
        .await
        .is_err());
    assert!(
        tokio::time::timeout(StdDuration::from_millis(1100), receiver.recv())
            .await
            .is_err()
    );
}
