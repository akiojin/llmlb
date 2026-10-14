use super::*;

/// T013 [US5]: seed_history_from_db が MinuteHistoryPoint を VecDeque に
/// 正しく投入できることを検証
#[tokio::test]
async fn test_seed_history_from_db() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;

    let now = Utc::now();
    let minute1 = super::align_to_minute(now - chrono::Duration::minutes(5));
    let minute2 = super::align_to_minute(now - chrono::Duration::minutes(3));

    let points = vec![
        crate::db::request_history::MinuteHistoryPoint {
            minute: minute1.to_rfc3339(),
            success_count: 10,
            error_count: 2,
        },
        crate::db::request_history::MinuteHistoryPoint {
            minute: minute2.to_rfc3339(),
            success_count: 5,
            error_count: 1,
        },
    ];

    load_manager.seed_history_from_db(points).await;

    let history = load_manager.request_history().await;
    // VecDeque には seed したデータが含まれる
    let total_success: u64 = history.iter().map(|h| h.success).sum();
    let total_error: u64 = history.iter().map(|h| h.error).sum();
    assert_eq!(total_success, 15, "seeded success count");
    assert_eq!(total_error, 3, "seeded error count");
}

// ===== align_to_minute テスト =====

#[test]
fn align_to_minute_strips_seconds_and_nanos() {
    let ts = chrono::TimeZone::with_ymd_and_hms(&Utc, 2025, 6, 15, 10, 30, 45).unwrap();
    let aligned = super::align_to_minute(ts);
    assert_eq!(aligned.second(), 0);
    assert_eq!(aligned.nanosecond(), 0);
    assert_eq!(aligned.minute(), 30);
    assert_eq!(aligned.hour(), 10);
}

#[test]
fn align_to_minute_already_aligned() {
    let ts = chrono::TimeZone::with_ymd_and_hms(&Utc, 2025, 1, 1, 0, 0, 0).unwrap();
    let aligned = super::align_to_minute(ts);
    assert_eq!(aligned, ts);
}

// ===== prune_history テスト =====

#[test]
fn prune_history_removes_old_entries() {
    let now = super::align_to_minute(Utc::now());
    let mut history = std::collections::VecDeque::new();
    // 120分前のエントリ（60分窓より古い）
    let old_minute = now - chrono::Duration::minutes(120);
    history.push_back(RequestHistoryPoint {
        minute: old_minute,
        success: 10,
        error: 1,
    });
    // 30分前のエントリ（窓内）
    let recent_minute = now - chrono::Duration::minutes(30);
    history.push_back(RequestHistoryPoint {
        minute: recent_minute,
        success: 5,
        error: 0,
    });

    super::prune_history(&mut history, now);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].minute, recent_minute);
}

#[test]
fn prune_history_keeps_all_within_window() {
    let now = super::align_to_minute(Utc::now());
    let mut history = std::collections::VecDeque::new();
    for i in 0..5 {
        history.push_back(RequestHistoryPoint {
            minute: now - chrono::Duration::minutes(i),
            success: 1,
            error: 0,
        });
    }
    super::prune_history(&mut history, now);
    assert_eq!(history.len(), 5);
}

#[test]
fn prune_history_empty_is_noop() {
    let now = super::align_to_minute(Utc::now());
    let mut history = std::collections::VecDeque::new();
    super::prune_history(&mut history, now);
    assert!(history.is_empty());
}

// ===== new_history_point テスト =====

#[test]
fn new_history_point_success() {
    let now = super::align_to_minute(Utc::now());
    let point = super::new_history_point(now, RequestOutcome::Success);
    assert_eq!(point.minute, now);
    assert_eq!(point.success, 1);
    assert_eq!(point.error, 0);
}

#[test]
fn new_history_point_error() {
    let now = super::align_to_minute(Utc::now());
    let point = super::new_history_point(now, RequestOutcome::Error);
    assert_eq!(point.minute, now);
    assert_eq!(point.success, 0);
    assert_eq!(point.error, 1);
}

#[test]
fn new_history_point_queued() {
    let now = super::align_to_minute(Utc::now());
    let point = super::new_history_point(now, RequestOutcome::Queued);
    assert_eq!(point.minute, now);
    assert_eq!(point.success, 0);
    assert_eq!(point.error, 0);
}

// ===== increment_history テスト =====

#[test]
fn increment_history_success() {
    let mut point = RequestHistoryPoint {
        minute: Utc::now(),
        success: 5,
        error: 2,
    };
    super::increment_history(&mut point, RequestOutcome::Success);
    assert_eq!(point.success, 6);
    assert_eq!(point.error, 2);
}

#[test]
fn increment_history_error() {
    let mut point = RequestHistoryPoint {
        minute: Utc::now(),
        success: 5,
        error: 2,
    };
    super::increment_history(&mut point, RequestOutcome::Error);
    assert_eq!(point.success, 5);
    assert_eq!(point.error, 3);
}

#[test]
fn increment_history_queued_no_change() {
    let mut point = RequestHistoryPoint {
        minute: Utc::now(),
        success: 5,
        error: 2,
    };
    super::increment_history(&mut point, RequestOutcome::Queued);
    assert_eq!(point.success, 5);
    assert_eq!(point.error, 2);
}

// ===== fill_history テスト =====

#[test]
fn fill_history_fills_gaps() {
    let now = super::align_to_minute(Utc::now());
    let mut map = std::collections::HashMap::new();
    // 10分前にデータがある
    let ten_ago = now - chrono::Duration::minutes(10);
    map.insert(
        ten_ago,
        RequestHistoryPoint {
            minute: ten_ago,
            success: 42,
            error: 0,
        },
    );

    let result = super::fill_history(now, &mut map);
    // REQUEST_HISTORY_WINDOW_MINUTESは60なので、結果は60エントリ
    assert_eq!(
        result.len(),
        crate::balancer::types::REQUEST_HISTORY_WINDOW_MINUTES as usize
    );
    // 10分前のエントリには success=42 が含まれる
    let ten_ago_entry = result.iter().find(|p| p.minute == ten_ago);
    assert!(ten_ago_entry.is_some());
    assert_eq!(ten_ago_entry.unwrap().success, 42);
    // 他のエントリは success=0, error=0
    let zero_entries: Vec<_> = result.iter().filter(|p| p.minute != ten_ago).collect();
    for e in zero_entries {
        assert_eq!(e.success, 0);
        assert_eq!(e.error, 0);
    }
}

// ===== metrics_history テスト =====

#[tokio::test]
async fn metrics_history_unknown_endpoint_returns_error() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let result = load_manager.metrics_history(Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn metrics_history_empty_for_no_metrics() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, endpoint_id) = setup_test_load_manager().await;
    let history = load_manager.metrics_history(endpoint_id).await.unwrap();
    assert!(history.is_empty());
}

// ===== record_request_history テスト =====

#[tokio::test]
async fn record_request_history_creates_entry() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let ts = Utc::now();
    load_manager
        .record_request_history(RequestOutcome::Success, ts)
        .await;
    let history = load_manager.request_history().await;
    let total_success: u64 = history.iter().map(|h| h.success).sum();
    assert_eq!(total_success, 1);
}

#[tokio::test]
async fn record_request_history_same_minute_aggregates() {
    let _lock = TEST_LOCK.lock().await;
    let (load_manager, _) = setup_test_load_manager().await;
    let ts = Utc::now();
    load_manager
        .record_request_history(RequestOutcome::Success, ts)
        .await;
    load_manager
        .record_request_history(RequestOutcome::Success, ts)
        .await;
    load_manager
        .record_request_history(RequestOutcome::Error, ts)
        .await;

    let history = load_manager.request_history().await;
    let total_success: u64 = history.iter().map(|h| h.success).sum();
    let total_error: u64 = history.iter().map(|h| h.error).sum();
    assert_eq!(total_success, 2);
    assert_eq!(total_error, 1);
}

// ===== build_history_window テスト =====

#[test]
fn build_history_window_returns_full_window() {
    let history = std::collections::VecDeque::new();
    let result = super::build_history_window(&history);
    assert_eq!(
        result.len(),
        crate::balancer::types::REQUEST_HISTORY_WINDOW_MINUTES as usize
    );
}
