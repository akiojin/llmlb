use super::*;

// スケジュールループのテストは `start_paused` で仮想時間を使う。仮想時間はすべてのタスクが
// 待機状態になるまで進まないため、ループの判定はホストの負荷に関係なく待機の満了前に完了する (#754)。

/// スケジュールループが消費するまで待つ。消費された時点で戻り、上限は仮想時間で数える。
async fn wait_for_schedule_consumed(manager: &UpdateManager) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while manager.get_schedule().unwrap().is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("schedule should be consumed by the schedule loop");
}

/// 5 秒間隔のスケジュールループに、起動直後・5 秒後・10 秒後の 3 回判定させる。
async fn let_schedule_loop_tick_three_times() {
    tokio::time::sleep(Duration::from_secs(11)).await;
}

// =======================================================================
// T232: アイドル時適用トリガー — in_flight=0でスケジュール起動
// =======================================================================
#[tokio::test(start_paused = true)]
async fn idle_schedule_triggers_when_in_flight_zero() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate.clone());

    // Set up available state.
    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    // Create idle schedule.
    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Idle,
        scheduled_at: None,
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager
        .create_schedule(sched)
        .expect("schedule should be created");

    // No in-flight requests → in_flight == 0.
    assert_eq!(gate.in_flight(), 0);

    // Start schedule loop.
    manager.start_schedule_loop();

    wait_for_schedule_consumed(&manager).await;

    // Schedule should have been removed (triggered).
    assert!(
        manager.get_schedule().unwrap().is_none(),
        "schedule should be consumed after idle trigger"
    );

    // Apply request should have been triggered.
    let mode = manager.take_apply_request_mode();
    assert_eq!(
        mode,
        ApplyRequestMode::Normal,
        "idle schedule should trigger normal apply"
    );
}

#[tokio::test(start_paused = true)]
async fn idle_schedule_does_not_trigger_while_busy() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate.clone());

    // Set up available state.
    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    // Simulate in-flight request.
    let _guard = gate.begin_for_test();

    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Idle,
        scheduled_at: None,
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager
        .create_schedule(sched)
        .expect("schedule should be created");

    manager.start_schedule_loop();
    let_schedule_loop_tick_three_times().await;

    // Schedule should still exist (not triggered).
    assert!(
        manager.get_schedule().unwrap().is_some(),
        "schedule should remain while requests are in-flight"
    );

    // No apply request should be pending.
    let mode = manager.take_apply_request_mode();
    assert_eq!(
        mode,
        ApplyRequestMode::None,
        "should not trigger while busy"
    );
}

#[test]
fn scheduled_mode_requires_scheduled_at() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Scheduled,
        scheduled_at: None,
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };

    let err = manager
        .create_schedule(sched)
        .expect_err("scheduled mode without scheduled_at must be rejected");
    assert!(
        err.to_string()
            .contains("scheduled_at is required when mode is scheduled"),
        "unexpected error: {err}"
    );
}

// =======================================================================
// T233: 時刻指定適用トリガー — 指定時刻到達でドレイン開始
// =======================================================================
#[tokio::test(start_paused = true)]
async fn scheduled_time_triggers_when_past_due() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    // Schedule for 1 second ago (already past due).
    let scheduled_at = Utc::now() - chrono::Duration::seconds(1);
    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Scheduled,
        scheduled_at: Some(scheduled_at),
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager
        .create_schedule(sched)
        .expect("schedule should be created");

    manager.start_schedule_loop();

    wait_for_schedule_consumed(&manager).await;

    assert!(
        manager.get_schedule().unwrap().is_none(),
        "schedule should be consumed after scheduled_at"
    );

    let mode = manager.take_apply_request_mode();
    assert_eq!(
        mode,
        ApplyRequestMode::Normal,
        "scheduled trigger should request normal apply"
    );
}

#[tokio::test(start_paused = true)]
async fn scheduled_time_does_not_trigger_when_target_version_mismatch() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    {
        let mut state = available_state_with_payload(PayloadState::NotReady);
        if let UpdateState::Available { latest, .. } = &mut state {
            *latest = "4.5.2".to_string();
        }
        *manager.inner.state.write().await = state;
    }

    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Scheduled,
        scheduled_at: Some(Utc::now() - chrono::Duration::seconds(1)),
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager
        .create_schedule(sched)
        .expect("schedule should be created");

    manager.start_schedule_loop();
    let_schedule_loop_tick_three_times().await;

    assert!(
        manager.get_schedule().unwrap().is_some(),
        "schedule should remain when target version no longer matches latest"
    );
    assert_eq!(
        manager.take_apply_request_mode(),
        ApplyRequestMode::None,
        "target version mismatch must not trigger apply"
    );
}

#[tokio::test(start_paused = true)]
async fn malformed_scheduled_without_time_does_not_trigger() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    // Simulate malformed persisted data from an older version.
    let malformed = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Scheduled,
        scheduled_at: None,
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager.inner.schedule_store.save(&malformed).unwrap();

    manager.start_schedule_loop();
    let_schedule_loop_tick_three_times().await;

    assert!(
        manager.get_schedule().unwrap().is_some(),
        "malformed scheduled entry should not be consumed automatically"
    );
    assert_eq!(
        manager.take_apply_request_mode(),
        ApplyRequestMode::None,
        "malformed scheduled entry must never trigger apply"
    );
}

#[tokio::test(start_paused = true)]
async fn scheduled_time_does_not_trigger_before_time() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    {
        *manager.inner.state.write().await = available_state_with_payload(PayloadState::NotReady);
    }

    // Schedule for 60 seconds from now (far future, won't trigger in test).
    let scheduled_at = Utc::now() + chrono::Duration::seconds(60);
    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Scheduled,
        scheduled_at: Some(scheduled_at),
        scheduled_by: "admin".to_string(),
        target_version: "4.5.1".to_string(),
        created_at: Utc::now(),
    };
    manager
        .create_schedule(sched)
        .expect("schedule should be created");

    manager.start_schedule_loop();

    let_schedule_loop_tick_three_times().await;

    assert!(
        manager.get_schedule().unwrap().is_some(),
        "schedule should not trigger before scheduled_at"
    );
    assert_eq!(
        manager.take_apply_request_mode(),
        ApplyRequestMode::None,
        "should not trigger before time"
    );
}

// =======================================================================
// UpdateManager: cancel_schedule errors when no schedule exists
// =======================================================================
#[test]
fn cancel_schedule_errors_when_empty() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    let err = manager
        .cancel_schedule()
        .expect_err("should error when no schedule");
    assert!(err.to_string().contains("No schedule exists"));
}

// =======================================================================
// UpdateManager: create_schedule errors on duplicate
// =======================================================================
#[test]
fn create_schedule_errors_on_duplicate() {
    let gate = InferenceGate::default();
    let (manager, _tmp) = test_manager_with_gate(gate);

    let sched = schedule::UpdateSchedule {
        mode: schedule::ScheduleMode::Idle,
        scheduled_at: None,
        scheduled_by: "admin".to_string(),
        target_version: "5.0.0".to_string(),
        created_at: Utc::now(),
    };

    manager.create_schedule(sched.clone()).unwrap();

    let err = manager
        .create_schedule(sched)
        .expect_err("should error on duplicate schedule");
    assert!(err.to_string().contains("already exists"));
}
