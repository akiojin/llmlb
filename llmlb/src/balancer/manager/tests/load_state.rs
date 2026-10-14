use super::*;

// NOTE: SPEC-e8e9326eによりNodeRegistryは廃止されました。
// SPEC-f8e3a1b7によりNode型は削除され、Endpoint型に移行しました。
// compare_average_ms_orders_values テストは関数削除に伴い削除されました。

#[test]
fn effective_average_ms_prefers_metrics_value() {
    let timestamp = Utc::now();
    let state = EndpointLoadState {
        success_count: 5,
        total_latency_ms: 500,
        last_metrics: Some(HealthMetrics {
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
            active_requests: 1,
            total_requests: 5,
            average_response_time_ms: Some(80.0),
            timestamp,
        }),
        ..Default::default()
    };

    assert_eq!(state.effective_average_ms(), Some(80.0));
}

// SPEC-f8e3a1b7: wait_for_ready_with_timeout / admission_control テストは削除されました

#[test]
fn test_node_load_state_token_accumulation() {
    let mut state = EndpointLoadState::default();

    assert_eq!(state.total_input_tokens, 0);
    assert_eq!(state.total_output_tokens, 0);
    assert_eq!(state.total_tokens, 0);

    state.total_input_tokens += 100;
    state.total_output_tokens += 50;
    state.total_tokens += 150;

    assert_eq!(state.total_input_tokens, 100);
    assert_eq!(state.total_output_tokens, 50);
    assert_eq!(state.total_tokens, 150);

    state.total_input_tokens += 200;
    state.total_output_tokens += 100;
    state.total_tokens += 300;

    assert_eq!(state.total_input_tokens, 300);
    assert_eq!(state.total_output_tokens, 150);
    assert_eq!(state.total_tokens, 450);
}

#[test]
fn test_node_load_state_average_tokens_per_request() {
    let state = EndpointLoadState {
        total_assigned: 10,
        total_input_tokens: 1000,
        total_output_tokens: 500,
        total_tokens: 1500,
        ..Default::default()
    };

    let avg = if state.total_assigned > 0 {
        state.total_tokens as f32 / state.total_assigned as f32
    } else {
        0.0
    };
    assert_eq!(avg, 150.0);
}

// ===== 追加テスト: LoadManager 基本機能 =====

#[test]
fn endpoint_load_state_default_values() {
    let state = EndpointLoadState::default();
    assert_eq!(state.assigned_active, 0);
    assert_eq!(state.total_assigned, 0);
    assert_eq!(state.success_count, 0);
    assert_eq!(state.error_count, 0);
    assert_eq!(state.total_latency_ms, 0);
    assert!(!state.initializing);
    assert!(state.ready_models.is_none());
    assert!(state.last_metrics.is_none());
    assert!(state.metrics_history.is_empty());
    assert_eq!(state.total_input_tokens, 0);
    assert_eq!(state.total_output_tokens, 0);
    assert_eq!(state.total_tokens, 0);
}

#[test]
fn endpoint_load_state_combined_active_no_metrics() {
    let state = EndpointLoadState {
        assigned_active: 3,
        ..Default::default()
    };
    assert_eq!(state.combined_active(), 3);
}

#[test]
fn endpoint_load_state_combined_active_metrics_higher() {
    let state = EndpointLoadState {
        assigned_active: 2,
        last_metrics: Some(HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 7,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: Utc::now(),
        }),
        ..Default::default()
    };
    assert_eq!(state.combined_active(), 7);
}

#[test]
fn endpoint_load_state_combined_active_assigned_higher() {
    let state = EndpointLoadState {
        assigned_active: 10,
        last_metrics: Some(HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 3,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: Utc::now(),
        }),
        ..Default::default()
    };
    assert_eq!(state.combined_active(), 10);
}

#[test]
fn endpoint_load_state_average_latency_ms_no_completed() {
    let state = EndpointLoadState::default();
    assert!(state.average_latency_ms().is_none());
}

#[test]
fn endpoint_load_state_average_latency_ms_with_data() {
    let state = EndpointLoadState {
        success_count: 8,
        error_count: 2,
        total_latency_ms: 1000,
        ..Default::default()
    };
    let avg = state.average_latency_ms().unwrap();
    assert!((avg - 100.0).abs() < 0.01, "expected 100.0, got {avg}");
}

#[test]
fn endpoint_load_state_is_stale_no_metrics() {
    let state = EndpointLoadState::default();
    assert!(state.is_stale(Utc::now()));
}

#[test]
fn endpoint_load_state_is_stale_fresh() {
    let now = Utc::now();
    let state = EndpointLoadState {
        last_metrics: Some(HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: now,
        }),
        ..Default::default()
    };
    assert!(!state.is_stale(now));
}

#[test]
fn endpoint_load_state_effective_average_ms_no_data() {
    let state = EndpointLoadState::default();
    assert!(state.effective_average_ms().is_none());
}

#[test]
fn endpoint_load_state_effective_average_ms_fallback_to_computed() {
    let now = Utc::now();
    let state = EndpointLoadState {
        success_count: 5,
        error_count: 0,
        total_latency_ms: 500,
        last_metrics: Some(HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: now,
        }),
        ..Default::default()
    };
    let avg = state.effective_average_ms().unwrap();
    assert!(
        (avg - 100.0).abs() < 0.01,
        "fallback to computed: expected 100.0, got {avg}"
    );
}

#[test]
fn endpoint_load_state_last_updated_none() {
    let state = EndpointLoadState::default();
    assert!(state.last_updated().is_none());
}

#[test]
fn endpoint_load_state_last_updated_some() {
    let now = Utc::now();
    let state = EndpointLoadState {
        last_metrics: Some(HealthMetrics {
            endpoint_id: Uuid::new_v4(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            total_requests: 0,
            average_response_time_ms: None,
            timestamp: now,
        }),
        ..Default::default()
    };
    assert_eq!(state.last_updated(), Some(now));
}
