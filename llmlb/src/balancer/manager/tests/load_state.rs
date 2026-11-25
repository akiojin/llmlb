use super::*;

// NOTE: SPEC-e8e9326eによりNodeRegistryは廃止されました。
// SPEC-f8e3a1b7によりNode型は削除され、Endpoint型に移行しました。
// compare_average_ms_orders_values テストは関数削除に伴い削除されました。
// Issue #982: heartbeat メトリクス（last_metrics / combined_active / is_stale /
// effective_average_ms）は撤去され、負荷状態はゲートウェイ観測値のみになった。

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
    assert_eq!(state.total_input_tokens, 0);
    assert_eq!(state.total_output_tokens, 0);
    assert_eq!(state.total_tokens, 0);
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
