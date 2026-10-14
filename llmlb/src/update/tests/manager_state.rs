use super::*;

#[tokio::test]
async fn record_check_failure_preserves_available_payload() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    let ready_payload = PayloadState::Ready {
        kind: PayloadKind::Portable {
            binary_path: "/tmp/llmlb-new".to_string(),
        },
    };

    {
        *manager.inner.state.write().await = available_state_with_payload(ready_payload.clone());
    }

    manager
        .record_check_failure("temporary network outage".to_string())
        .await;

    match manager.state().await {
        UpdateState::Available {
            latest, payload, ..
        } => {
            assert_eq!(latest, "4.5.1");
            assert_eq!(payload, ready_payload);
        }
        other => panic!("expected available state, got {other:?}"),
    }
}

#[tokio::test]
async fn record_check_failure_transitions_non_available_to_failed() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    {
        *manager.inner.state.write().await = UpdateState::UpToDate { checked_at: None };
    }

    manager
        .record_check_failure("check failed".to_string())
        .await;

    match manager.state().await {
        UpdateState::Failed {
            latest,
            release_url,
            message,
            ..
        } => {
            assert_eq!(latest, None);
            assert_eq!(release_url, None);
            assert_eq!(message, "check failed");
        }
        other => panic!("expected failed state, got {other:?}"),
    }
}

#[tokio::test]
async fn request_apply_normal_reports_not_queued_when_ready_and_idle() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::Ready {
            kind: PayloadKind::Portable {
                binary_path: "/tmp/llmlb-new".to_string(),
            },
        });
    }

    let queued = manager.request_apply_normal().await;
    assert!(!queued);
    assert_eq!(manager.take_apply_request_mode(), ApplyRequestMode::Normal);
}

#[tokio::test]
async fn request_apply_normal_reports_queued_when_payload_not_ready() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    let queued = manager.request_apply_normal().await;
    assert!(queued);
    assert_eq!(manager.take_apply_request_mode(), ApplyRequestMode::Normal);
}

#[tokio::test]
async fn request_apply_force_requires_ready_payload() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    let err = manager
        .request_apply_force()
        .await
        .expect_err("force apply should fail when payload is not ready");
    assert!(err.to_string().contains("not ready"));
}

#[tokio::test]
async fn request_apply_force_promotes_pending_normal_request() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::Ready {
            kind: PayloadKind::Portable {
                binary_path: "/tmp/llmlb-new".to_string(),
            },
        });
    }

    manager.request_apply();
    let dropped = manager
        .request_apply_force()
        .await
        .expect("force apply request should be accepted");
    assert_eq!(dropped, 0);
    assert_eq!(manager.take_apply_request_mode(), ApplyRequestMode::Force);
}

// =======================================================================
// T212: レートリミット判定
// =======================================================================
#[tokio::test]
async fn rate_limit_rejects_within_60_seconds() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    // First call should succeed (not rate-limited).
    assert!(
        !manager.is_manual_check_rate_limited(),
        "first call should not be rate-limited"
    );
    manager.record_manual_check();

    // Immediate second call should be rate-limited.
    assert!(
        manager.is_manual_check_rate_limited(),
        "second call within 60s should be rate-limited"
    );
}

#[tokio::test]
async fn rate_limit_allows_after_cooldown() {
    use tokio::time;

    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .expect("create update manager");

    manager.record_manual_check();
    assert!(manager.is_manual_check_rate_limited());

    // Advance time past 60 seconds.
    time::pause();
    time::advance(Duration::from_secs(61)).await;

    assert!(
        !manager.is_manual_check_rate_limited(),
        "should allow check after 60s cooldown"
    );
}

// =======================================================================
// T250: ドレインタイムアウト — タイムアウト超過でキャンセル＋ゲート再開＋failed遷移
// =======================================================================
#[tokio::test]
async fn drain_timeout_cancels_and_transitions_to_failed() {
    use tokio::time;

    time::pause();

    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate.clone());

    // Set up available state with ready payload.
    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::Ready {
            kind: PayloadKind::Portable {
                binary_path: "/tmp/llmlb-new".to_string(),
            },
        });
    }

    // Simulate an in-flight request that never completes.
    let _guard = gate.begin_for_test();

    // Start apply_flow in a task — it will try to drain.
    let mgr = manager.clone();
    let apply_task = tokio::spawn(async move { mgr.apply_flow(ApplyRequestMode::Normal).await });

    // Let the drain start.
    time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    // Verify we're in Draining state.
    let state = manager.state().await;
    assert!(
        matches!(state, UpdateState::Draining { .. }),
        "expected draining, got {state:?}"
    );

    // Advance time past the drain timeout (300s).
    time::advance(Duration::from_secs(301)).await;
    tokio::task::yield_now().await;

    // apply_flow should return an error.
    let result = apply_task.await.expect("task should complete");
    assert!(result.is_err(), "apply_flow should fail on drain timeout");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("timed out"),
        "error should mention timeout: {err_msg}"
    );

    // State should be Failed.
    let state = manager.state().await;
    match &state {
        UpdateState::Failed { message, .. } => {
            assert!(
                message.contains("timed out"),
                "failed message should mention timeout: {message}"
            );
        }
        other => panic!("expected failed state, got {other:?}"),
    }

    // Gate should no longer be rejecting.
    assert!(
        !gate.is_rejecting(),
        "gate should stop rejecting after drain timeout"
    );
}

// T250 supplemental: drain that completes before timeout succeeds.
#[tokio::test]
async fn drain_completes_before_timeout() {
    use tokio::time;

    time::pause();

    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate.clone());

    // Set up available state with ready payload.
    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::Ready {
            kind: PayloadKind::Portable {
                binary_path: "/tmp/llmlb-new".to_string(),
            },
        });
    }

    // Simulate an in-flight request.
    let guard = gate.begin_for_test();

    let mgr = manager.clone();
    let apply_task = tokio::spawn(async move { mgr.apply_flow(ApplyRequestMode::Normal).await });

    // Let drain start.
    time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    // Complete the request before timeout.
    drop(guard);
    time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    // apply_flow will fail because it tries to spawn a real binary,
    // but it should NOT fail due to timeout.
    let result = apply_task.await.expect("task should complete");
    // The error (if any) should be about spawning, not timeout.
    if let Err(e) = &result {
        assert!(
            !e.to_string().contains("timed out"),
            "should not time out: {e}"
        );
    }

    // State should NOT be Failed due to timeout.
    let state = manager.state().await;
    assert!(
        !matches!(
            &state,
            UpdateState::Failed { message, .. } if message.contains("timed out")
        ),
        "should not be in timeout-failed state: {state:?}"
    );
}

// =======================================================================
// UpdateManager: state transitions
// =======================================================================
#[tokio::test]
async fn update_manager_initial_state_is_up_to_date() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    match manager.state().await {
        UpdateState::UpToDate { checked_at } => {
            assert!(checked_at.is_none());
        }
        other => panic!("expected up_to_date, got {other:?}"),
    }
}

#[tokio::test]
async fn set_applying_state_updates_correctly() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let started = Utc::now();
    manager
        .set_applying_state(
            "5.0.0",
            ApplyMethod::PortableReplace,
            ApplyPhase::Starting,
            started,
            None,
        )
        .await;

    match manager.state().await {
        UpdateState::Applying {
            latest,
            method,
            phase,
            timeout_at,
            ..
        } => {
            assert_eq!(latest, "5.0.0");
            assert_eq!(method, ApplyMethod::PortableReplace);
            assert_eq!(phase, ApplyPhase::Starting);
            assert!(timeout_at.is_none());
        }
        other => panic!("expected applying, got {other:?}"),
    }
}

#[tokio::test]
async fn set_payload_error_sets_error_on_available_state() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    manager
        .set_payload_error("download timeout".to_string())
        .await;

    match manager.state().await {
        UpdateState::Available { payload, .. } => {
            assert_eq!(
                payload,
                PayloadState::Error {
                    message: "download timeout".to_string()
                }
            );
        }
        other => panic!("expected available with error payload, got {other:?}"),
    }
}

#[tokio::test]
async fn set_payload_error_noop_on_non_available_state() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    // State is UpToDate (default), set_payload_error should be a no-op
    manager.set_payload_error("some error".to_string()).await;

    match manager.state().await {
        UpdateState::UpToDate { .. } => {} // unchanged
        other => panic!("expected up_to_date unchanged, got {other:?}"),
    }
}

// =======================================================================
// UpdateManager: require_ready_payload
// =======================================================================
#[tokio::test]
async fn require_ready_payload_returns_kind_when_ready() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let expected_kind = PayloadKind::Portable {
        binary_path: "/tmp/new-llmlb".to_string(),
    };
    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::Ready {
            kind: expected_kind.clone(),
        });
    }

    let kind = manager.require_ready_payload().await.unwrap();
    assert_eq!(kind, expected_kind);
}

#[tokio::test]
async fn require_ready_payload_errors_when_not_ready() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    let err = manager.require_ready_payload().await.unwrap_err();
    assert!(err.to_string().contains("not ready"));
}

#[tokio::test]
async fn require_ready_payload_errors_when_no_update() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let err = manager.require_ready_payload().await.unwrap_err();
    assert!(err.to_string().contains("No update is available"));
}

// =======================================================================
// UpdateManager: validate_force_apply_request
// =======================================================================
#[tokio::test]
async fn validate_force_apply_rejects_draining_state() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    {
        *manager.inner.state.write().await = UpdateState::Draining {
            latest: "5.0.0".to_string(),
            in_flight: 3,
            requested_at: Utc::now(),
            timeout_at: Utc::now() + chrono::Duration::seconds(300),
        };
    }

    let err = manager.validate_force_apply_request().await.unwrap_err();
    assert!(err.to_string().contains("already in progress"));
}

#[tokio::test]
async fn validate_force_apply_rejects_up_to_date() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let err = manager.validate_force_apply_request().await.unwrap_err();
    assert!(err.to_string().contains("No update is available"));
}

// =======================================================================
// UpdateManager: apply_cache
// =======================================================================
#[tokio::test]
async fn apply_cache_empty_version_stays_up_to_date() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let cache = UpdateCacheFile {
        last_checked_at: Utc::now(),
        latest_version: Some("".to_string()),
        release_url: None,
        portable_asset_url: None,
        installer_asset_url: None,
    };
    manager.apply_cache(cache).await.unwrap();

    match manager.state().await {
        UpdateState::UpToDate { checked_at } => {
            assert!(checked_at.is_some());
        }
        other => panic!("expected up_to_date, got {other:?}"),
    }
}

#[tokio::test]
async fn apply_cache_none_version_stays_up_to_date() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let cache = UpdateCacheFile {
        last_checked_at: Utc::now(),
        latest_version: None,
        release_url: None,
        portable_asset_url: None,
        installer_asset_url: None,
    };
    manager.apply_cache(cache).await.unwrap();

    assert!(matches!(
        manager.state().await,
        UpdateState::UpToDate { .. }
    ));
}

#[tokio::test]
async fn apply_cache_invalid_version_errors() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    let cache = UpdateCacheFile {
        last_checked_at: Utc::now(),
        latest_version: Some("not-semver".to_string()),
        release_url: None,
        portable_asset_url: None,
        installer_asset_url: None,
    };
    let err = manager.apply_cache(cache).await;
    assert!(err.is_err());
}

// =======================================================================
// UpdateManager: record_check_failure from Draining and Applying states
// =======================================================================
#[tokio::test]
async fn record_check_failure_from_draining_preserves_latest() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    {
        *manager.inner.state.write().await = UpdateState::Draining {
            latest: "5.0.0".to_string(),
            in_flight: 2,
            requested_at: Utc::now(),
            timeout_at: Utc::now() + chrono::Duration::seconds(300),
        };
    }

    manager
        .record_check_failure("error during drain".to_string())
        .await;

    match manager.state().await {
        UpdateState::Failed {
            latest, message, ..
        } => {
            assert_eq!(latest, Some("5.0.0".to_string()));
            assert_eq!(message, "error during drain");
        }
        other => panic!("expected failed, got {other:?}"),
    }
}

#[tokio::test]
async fn record_check_failure_from_applying_preserves_latest() {
    let manager = UpdateManager::new(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
    )
    .unwrap();

    {
        *manager.inner.state.write().await = UpdateState::Applying {
            latest: "5.0.0".to_string(),
            method: ApplyMethod::PortableReplace,
            phase: ApplyPhase::Starting,
            phase_message: "test".to_string(),
            started_at: Utc::now(),
            timeout_at: None,
        };
    }

    manager
        .record_check_failure("error during apply".to_string())
        .await;

    match manager.state().await {
        UpdateState::Failed { latest, .. } => {
            assert_eq!(latest, Some("5.0.0".to_string()));
        }
        other => panic!("expected failed, got {other:?}"),
    }
}

// =======================================================================
// UpdateManager: start_background_tasks is idempotent
// =======================================================================
#[tokio::test]
async fn start_background_tasks_is_idempotent() {
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());

    // First call should not panic
    manager.start_background_tasks();
    // Second call should be a no-op (idempotent)
    manager.start_background_tasks();
    // Just verify it doesn't panic or deadlock
    assert!(manager.inner.started.load(Ordering::SeqCst));
}

// =======================================================================
// UpdateManager: history roundtrip
// =======================================================================
#[test]
fn record_and_get_history() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    assert!(manager.get_history().is_empty());

    manager.record_history(history::HistoryEntry {
        kind: history::HistoryEventKind::Applied,
        version: "5.0.0".to_string(),
        message: Some("applied successfully".to_string()),
        timestamp: Utc::now(),
    });

    let h = manager.get_history();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].version, "5.0.0");
}

// =======================================================================
// UpdateManager: new_with_data_dir isolation
// =======================================================================
#[test]
fn new_with_data_dir_uses_temp_dir() {
    let dir = tempfile::tempdir().unwrap();
    let manager = UpdateManager::new_with_data_dir(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
        dir.path(),
    )
    .unwrap();

    assert!(manager.inner.cache_path.starts_with(dir.path()));
    assert!(manager.inner.updates_dir.starts_with(dir.path()));
}

#[tokio::test]
async fn check_only_github_error_cache_fallback() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    // GitHub API が 429 を返すようモック
    Mock::given(method("GET"))
        .and(path("/repos/test-owner/test-repo/releases/latest"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&mock_server)
        .await;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let manager = UpdateManager::new_with_data_dir_and_config(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
        tmp.path(),
        Some(mock_server.uri()),
    )
    .expect("create update manager");

    // --- ケース1: キャッシュなし → エラーが返る ---
    assert!(
        !manager.inner.cache_path.exists(),
        "cache should not exist in fresh temp dir"
    );
    let result = manager.check_only(true).await;
    assert!(
        result.is_err(),
        "check_only should fail when no cache and GitHub returns 429"
    );

    // --- ケース2: キャッシュあり → フォールバックで成功 ---
    save_cache(
        &manager.inner.cache_path,
        UpdateCacheFile {
            last_checked_at: Utc::now(),
            latest_version: Some("99.0.0".to_string()),
            release_url: Some(
                "https://github.com/test-owner/test-repo/releases/tag/v99.0.0".to_string(),
            ),
            portable_asset_url: Some("https://example.com/portable.tar.gz".to_string()),
            installer_asset_url: None,
        },
    )
    .expect("save cache");

    // force=true でもキャッシュフォールバックすべき
    let state = manager
        .check_only(true)
        .await
        .expect("check_only should succeed via cache fallback");

    match &state {
        UpdateState::Available { latest, .. } => {
            assert_eq!(latest, "99.0.0");
        }
        other => panic!("expected Available from cache fallback, got {other:?}"),
    }

    // --- ケース3: 既にAvailable(payload=Ready)なら状態を保持 ---
    {
        let mut st = manager.inner.state.write().await;
        *st = UpdateState::Available {
            current: "5.0.0".to_string(),
            latest: "99.0.0".to_string(),
            release_url: "https://example.com/release".to_string(),
            portable_asset_url: Some("https://example.com/portable.tar.gz".to_string()),
            installer_asset_url: None,
            payload: PayloadState::Ready {
                kind: PayloadKind::Portable {
                    binary_path: "/tmp/llmlb-new".to_string(),
                },
            },
            checked_at: Utc::now(),
        };
    }

    let state = manager
        .check_only(true)
        .await
        .expect("check_only should preserve existing Available state");

    match &state {
        UpdateState::Available { payload, .. } => {
            assert!(
                matches!(payload, PayloadState::Ready { .. }),
                "payload should remain Ready, got {payload:?}"
            );
        }
        other => panic!("expected Available with Ready payload, got {other:?}"),
    }
}

/// Regression test: `check_and_maybe_download` must not deadlock by holding
/// the state write guard across the `ensure_payload_ready().await` call.
///
/// Before the fix, the write guard in `check_and_maybe_download` was not
/// dropped before calling `ensure_payload_ready`, which tried to acquire a
/// read lock on the same `RwLock` — causing an irrecoverable deadlock.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_and_maybe_download_does_not_deadlock() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    // Return a release with a version higher than current.
    let release_json = serde_json::json!({
        "tag_name": "v99.0.0",
        "html_url": "https://github.com/test-owner/test-repo/releases/tag/v99.0.0",
        "assets": []
    });
    Mock::given(method("GET"))
        .and(path("/repos/akiojin/llmlb/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&release_json))
        .mount(&mock_server)
        .await;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let manager = UpdateManager::new_with_data_dir_and_config(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
        tmp.path(),
        Some(mock_server.uri()),
    )
    .expect("create update manager");

    // check_and_maybe_download with force=true triggers the code path
    // that previously deadlocked. Use a timeout to detect deadlocks.
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        manager.check_and_maybe_download(true),
    )
    .await;

    assert!(
        result.is_ok(),
        "check_and_maybe_download should not deadlock (timed out)"
    );

    // Verify we can still read state (would hang if write lock is held).
    let state = tokio::time::timeout(Duration::from_secs(2), manager.state())
        .await
        .expect("state() should not deadlock");

    match &state {
        UpdateState::Available { latest, .. } => {
            assert_eq!(latest, "99.0.0");
        }
        other => panic!("expected Available state, got {other:?}"),
    }
}
