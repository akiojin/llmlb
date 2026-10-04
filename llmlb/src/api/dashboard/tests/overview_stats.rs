use super::*;

/// T010 [US3]: collect_stats のフォールバック計算テスト
/// インメモリavg_response_time_msがNoneの場合、オンラインエンドポイントの
/// latency_msから平均値を計算する
#[test]
fn test_avg_response_time_fallback_from_latency() {
    // (1) summary の average_response_time_ms が None
    // (2) オンラインエンドポイント2つ (latency_ms=100, 200)
    let mut ep1 = Endpoint::new(
        "EP1".to_string(),
        "http://localhost:8001".to_string(),
        EndpointType::Xllm,
    );
    ep1.status = EndpointStatus::Online;
    ep1.latency_ms = Some(100);

    let mut ep2 = Endpoint::new(
        "EP2".to_string(),
        "http://localhost:8002".to_string(),
        EndpointType::Xllm,
    );
    ep2.status = EndpointStatus::Online;
    ep2.latency_ms = Some(200);

    let endpoints = vec![ep1, ep2];

    // (3) 結果は 150.0（平均値）
    let result = fallback_avg_response_time(None, &endpoints);
    assert_eq!(result, Some(150.0));
}

/// T010 追加シナリオ: 全エンドポイントがオフラインの場合は None のまま
#[test]
fn test_avg_response_time_fallback_all_offline() {
    let mut ep1 = Endpoint::new(
        "EP1".to_string(),
        "http://localhost:8001".to_string(),
        EndpointType::Xllm,
    );
    ep1.status = EndpointStatus::Offline;
    ep1.latency_ms = Some(100);

    let endpoints = vec![ep1];

    let result = fallback_avg_response_time(None, &endpoints);
    assert_eq!(result, None);
}

/// T010 追加シナリオ: summary に値がある場合はフォールバックしない
#[test]
fn test_avg_response_time_no_fallback_when_present() {
    let endpoints = vec![];
    let result = fallback_avg_response_time(Some(42.0), &endpoints);
    assert_eq!(result, Some(42.0));
}

// ===== DashboardStats serialization tests =====

#[test]
fn test_dashboard_stats_serialization() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 5,
        online_nodes: 3,
        pending_nodes: 1,
        registering_nodes: 0,
        offline_nodes: 1,
        total_requests: 1000,
        successful_requests: 950,
        failed_requests: 50,
        total_active_requests: 2,
        queued_requests: 0,
        average_response_time_ms: Some(150.5),
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: None,
        last_seen_at: None,
        openai_key_present: true,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: 100000,
        total_output_tokens: 50000,
        total_tokens: 150000,
    };

    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["total_runtimes"], 5);
    assert_eq!(json["online_runtimes"], 3);
    assert_eq!(json["pending_runtimes"], 1);
    assert_eq!(json["registering_runtimes"], 0);
    assert_eq!(json["offline_runtimes"], 1);
    assert_eq!(json["total_requests"], 1000);
    assert_eq!(json["successful_requests"], 950);
    assert_eq!(json["failed_requests"], 50);
    assert_eq!(json["total_active_requests"], 2);
    assert_eq!(json["queued_requests"], 0);
    assert_eq!(json["average_response_time_ms"], 150.5);
    assert!(json["average_gpu_usage"].is_null());
    assert_eq!(json["openai_key_present"], true);
    assert_eq!(json["google_key_present"], false);
    assert_eq!(json["anthropic_key_present"], false);
    assert_eq!(json["total_input_tokens"], 100000);
    assert_eq!(json["total_output_tokens"], 50000);
    assert_eq!(json["total_tokens"], 150000);
}

#[test]
fn test_dashboard_stats_rename_fields() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 2,
        online_nodes: 1,
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: 1,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        total_active_requests: 0,
        queued_requests: 0,
        average_response_time_ms: None,
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: None,
        last_seen_at: None,
        openai_key_present: false,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
    };

    let json = serde_json::to_value(&stats).unwrap();
    // Verify the serde rename_all is applied (total_runtimes, not total_nodes)
    assert!(json.get("total_runtimes").is_some());
    assert!(json.get("online_runtimes").is_some());
    assert!(json.get("pending_runtimes").is_some());
    assert!(json.get("offline_runtimes").is_some());
}

// ===== DashboardEndpoint tests =====

#[test]
fn test_dashboard_endpoint_serialization() {
    use super::DashboardEndpoint;
    use crate::types::endpoint::{EndpointStatus, EndpointType};

    let endpoint = DashboardEndpoint {
        id: uuid::Uuid::nil(),
        name: "test-endpoint".to_string(),
        base_url: "http://localhost:8080".to_string(),
        status: EndpointStatus::Online,
        endpoint_type: EndpointType::Xllm,
        health_check_interval_secs: 30,
        inference_timeout_secs: 120,
        latency_ms: Some(45),
        last_seen: None,
        last_error: None,
        error_count: 0,
        registered_at: chrono::Utc::now(),
        notes: None,
        model_count: 3,
        total_requests: 100,
        successful_requests: 95,
        failed_requests: 5,
    };

    let json = serde_json::to_value(&endpoint).unwrap();
    assert_eq!(json["name"], "test-endpoint");
    assert_eq!(json["base_url"], "http://localhost:8080");
    assert_eq!(json["health_check_interval_secs"], 30);
    assert_eq!(json["inference_timeout_secs"], 120);
    assert_eq!(json["latency_ms"], 45);
    assert_eq!(json["error_count"], 0);
    assert_eq!(json["model_count"], 3);
    assert_eq!(json["total_requests"], 100);
    assert_eq!(json["successful_requests"], 95);
    assert_eq!(json["failed_requests"], 5);
}

// ===== PersistedRequestTotals / PersistedTokenTotals / PersistedTotalsCache tests =====

#[test]
fn test_persisted_request_totals_default() {
    use super::PersistedRequestTotals;
    let defaults = PersistedRequestTotals::default();
    assert_eq!(defaults.total_requests, 0);
    assert_eq!(defaults.successful_requests, 0);
    assert_eq!(defaults.failed_requests, 0);
}

#[test]
fn test_persisted_token_totals_default() {
    use super::PersistedTokenTotals;
    let defaults = PersistedTokenTotals::default();
    assert_eq!(defaults.total_input_tokens, 0);
    assert_eq!(defaults.total_output_tokens, 0);
    assert_eq!(defaults.total_tokens, 0);
}

#[test]
fn test_persisted_totals_cache_default() {
    use super::PersistedTotalsCache;
    let cache = PersistedTotalsCache::default();
    assert_eq!(cache.request_totals.total_requests, 0);
    assert_eq!(cache.token_totals.total_tokens, 0);
}

// ===== DashboardEndpoint equality tests =====

#[test]
fn test_dashboard_endpoint_equality() {
    use super::DashboardEndpoint;
    use crate::types::endpoint::{EndpointStatus, EndpointType};

    let ts = chrono::Utc::now();
    let ep1 = DashboardEndpoint {
        id: uuid::Uuid::nil(),
        name: "ep".to_string(),
        base_url: "http://localhost".to_string(),
        status: EndpointStatus::Online,
        endpoint_type: EndpointType::Xllm,
        health_check_interval_secs: 30,
        inference_timeout_secs: 120,
        latency_ms: None,
        last_seen: None,
        last_error: None,
        error_count: 0,
        registered_at: ts,
        notes: None,
        model_count: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
    };
    let ep2 = ep1.clone();
    assert_eq!(ep1, ep2);
}

// ===== DashboardStats equality =====

#[test]
fn test_dashboard_stats_equality() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 0,
        online_nodes: 0,
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        total_active_requests: 0,
        queued_requests: 0,
        average_response_time_ms: None,
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: None,
        last_seen_at: None,
        openai_key_present: false,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
    };
    let stats2 = stats.clone();
    assert_eq!(stats, stats2);
}

// ===== ModelTpsEntry serialization =====

#[test]
fn test_model_tps_entry_serialization() {
    use super::ModelTpsEntry;
    use crate::common::protocol::{TpsApiKind, TpsSource};

    let entry = ModelTpsEntry {
        model_id: "llama-3-8b".to_string(),
        api_kind: TpsApiKind::ChatCompletions,
        source: TpsSource::Production,
        tps: Some(42.5),
        request_count: 100,
        total_output_tokens: 5000,
        average_duration_ms: Some(200.0),
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["model_id"], "llama-3-8b");
    assert_eq!(json["tps"], 42.5);
    assert_eq!(json["request_count"], 100);
    assert_eq!(json["total_output_tokens"], 5000);
    assert_eq!(json["average_duration_ms"], 200.0);
}

// ===== fallback_avg_response_time with latency_ms None =====

#[test]
fn test_avg_response_time_fallback_online_no_latency() {
    let mut ep = Endpoint::new(
        "EP".to_string(),
        "http://localhost:8001".to_string(),
        EndpointType::Xllm,
    );
    ep.status = EndpointStatus::Online;
    ep.latency_ms = None;

    let result = fallback_avg_response_time(None, &[ep]);
    assert_eq!(result, None);
}

// ===== Additional unit tests for increased coverage =====

// --- DashboardStats extended tests ---

#[test]
fn test_dashboard_stats_all_none_optional_fields() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 0,
        online_nodes: 0,
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        total_active_requests: 0,
        queued_requests: 0,
        average_response_time_ms: None,
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: None,
        last_seen_at: None,
        openai_key_present: false,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert!(json["average_response_time_ms"].is_null());
    assert!(json["average_gpu_usage"].is_null());
    assert!(json["average_gpu_memory_usage"].is_null());
    assert!(json["last_metrics_updated_at"].is_null());
    assert!(json["last_registered_at"].is_null());
    assert!(json["last_seen_at"].is_null());
}

#[test]
fn test_dashboard_stats_with_gpu_metrics() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 1,
        online_nodes: 1,
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: 0,
        total_requests: 100,
        successful_requests: 90,
        failed_requests: 10,
        total_active_requests: 5,
        queued_requests: 2,
        average_response_time_ms: Some(250.0),
        average_gpu_usage: Some(85.5),
        average_gpu_memory_usage: Some(72.3),
        last_metrics_updated_at: Some(chrono::Utc::now()),
        last_registered_at: Some(chrono::Utc::now()),
        last_seen_at: Some(chrono::Utc::now()),
        openai_key_present: true,
        google_key_present: true,
        anthropic_key_present: true,
        total_input_tokens: 50000,
        total_output_tokens: 25000,
        total_tokens: 75000,
    };
    let json = serde_json::to_value(&stats).unwrap();
    let gpu_usage = json["average_gpu_usage"].as_f64().unwrap();
    assert!((gpu_usage - 85.5).abs() < 0.01);
    let gpu_mem = json["average_gpu_memory_usage"].as_f64().unwrap();
    assert!((gpu_mem - 72.3).abs() < 0.01);
    assert!(json["last_metrics_updated_at"].is_string());
    assert!(json["last_registered_at"].is_string());
    assert!(json["last_seen_at"].is_string());
}

#[test]
fn test_dashboard_stats_token_counts() {
    use super::DashboardStats;

    let stats = DashboardStats {
        total_nodes: 0,
        online_nodes: 0,
        pending_nodes: 0,
        registering_nodes: 0,
        offline_nodes: 0,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        total_active_requests: 0,
        queued_requests: 0,
        average_response_time_ms: None,
        average_gpu_usage: None,
        average_gpu_memory_usage: None,
        last_metrics_updated_at: None,
        last_registered_at: None,
        last_seen_at: None,
        openai_key_present: false,
        google_key_present: false,
        anthropic_key_present: false,
        total_input_tokens: u64::MAX,
        total_output_tokens: u64::MAX,
        total_tokens: u64::MAX,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["total_input_tokens"], u64::MAX);
    assert_eq!(json["total_output_tokens"], u64::MAX);
    assert_eq!(json["total_tokens"], u64::MAX);
}

// --- DashboardEndpoint extended tests ---

#[test]
fn test_dashboard_endpoint_with_error() {
    use super::DashboardEndpoint;
    use crate::types::endpoint::{EndpointStatus, EndpointType};

    let endpoint = DashboardEndpoint {
        id: uuid::Uuid::nil(),
        name: "error-endpoint".to_string(),
        base_url: "http://localhost:8080".to_string(),
        status: EndpointStatus::Offline,
        endpoint_type: EndpointType::Vllm,
        health_check_interval_secs: 60,
        inference_timeout_secs: 300,
        latency_ms: None,
        last_seen: Some(chrono::Utc::now()),
        last_error: Some("Connection refused".to_string()),
        error_count: 5,
        registered_at: chrono::Utc::now(),
        notes: Some("This endpoint has issues".to_string()),
        model_count: 0,
        total_requests: 50,
        successful_requests: 40,
        failed_requests: 10,
    };
    let json = serde_json::to_value(&endpoint).unwrap();
    assert_eq!(json["status"], "offline");
    assert_eq!(json["last_error"], "Connection refused");
    assert_eq!(json["error_count"], 5);
    assert_eq!(json["notes"], "This endpoint has issues");
}

#[test]
fn test_dashboard_endpoint_all_endpoint_types() {
    use super::DashboardEndpoint;
    use crate::types::endpoint::{EndpointStatus, EndpointType};

    let types = [
        EndpointType::Xllm,
        EndpointType::Vllm,
        EndpointType::Ollama,
        EndpointType::OpenaiCompatible,
    ];
    for ep_type in types {
        let endpoint = DashboardEndpoint {
            id: uuid::Uuid::new_v4(),
            name: format!("ep-{:?}", ep_type),
            base_url: "http://localhost".to_string(),
            status: EndpointStatus::Online,
            endpoint_type: ep_type,
            health_check_interval_secs: 30,
            inference_timeout_secs: 120,
            latency_ms: None,
            last_seen: None,
            last_error: None,
            error_count: 0,
            registered_at: chrono::Utc::now(),
            notes: None,
            model_count: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
        };
        let json = serde_json::to_value(&endpoint).unwrap();
        assert!(json["endpoint_type"].is_string());
    }
}

#[test]
fn test_dashboard_endpoint_clone() {
    use super::DashboardEndpoint;
    use crate::types::endpoint::{EndpointStatus, EndpointType};

    let endpoint = DashboardEndpoint {
        id: uuid::Uuid::nil(),
        name: "clone-test".to_string(),
        base_url: "http://localhost".to_string(),
        status: EndpointStatus::Online,
        endpoint_type: EndpointType::Xllm,
        health_check_interval_secs: 30,
        inference_timeout_secs: 120,
        latency_ms: Some(42),
        last_seen: None,
        last_error: None,
        error_count: 0,
        registered_at: chrono::Utc::now(),
        notes: None,
        model_count: 2,
        total_requests: 10,
        successful_requests: 8,
        failed_requests: 2,
    };
    let cloned = endpoint.clone();
    assert_eq!(endpoint, cloned);
}

// --- PersistedRequestTotals / PersistedTokenTotals extended tests ---

#[test]
fn test_persisted_request_totals_copy() {
    use super::PersistedRequestTotals;
    let totals = PersistedRequestTotals {
        total_requests: 100,
        successful_requests: 90,
        failed_requests: 10,
    };
    let copied = totals;
    assert_eq!(copied.total_requests, 100);
    assert_eq!(copied.successful_requests, 90);
    assert_eq!(copied.failed_requests, 10);
}

#[test]
fn test_persisted_token_totals_copy() {
    use super::PersistedTokenTotals;
    let totals = PersistedTokenTotals {
        total_input_tokens: 50000,
        total_output_tokens: 25000,
        total_tokens: 75000,
    };
    let copied = totals;
    assert_eq!(copied.total_input_tokens, 50000);
    assert_eq!(copied.total_output_tokens, 25000);
    assert_eq!(copied.total_tokens, 75000);
}

#[test]
fn test_persisted_totals_cache_copy() {
    use super::{PersistedRequestTotals, PersistedTokenTotals, PersistedTotalsCache};
    let cache = PersistedTotalsCache {
        request_totals: PersistedRequestTotals {
            total_requests: 10,
            successful_requests: 8,
            failed_requests: 2,
        },
        token_totals: PersistedTokenTotals {
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_tokens: 1500,
        },
    };
    let copied = cache;
    assert_eq!(copied.request_totals.total_requests, 10);
    assert_eq!(copied.token_totals.total_tokens, 1500);
}

// --- fallback_avg_response_time extended tests ---

#[test]
fn test_avg_response_time_fallback_mixed_status_endpoints() {
    let mut ep_online = Endpoint::new(
        "Online".to_string(),
        "http://localhost:8001".to_string(),
        EndpointType::Xllm,
    );
    ep_online.status = EndpointStatus::Online;
    ep_online.latency_ms = Some(300);

    let mut ep_offline = Endpoint::new(
        "Offline".to_string(),
        "http://localhost:8002".to_string(),
        EndpointType::Xllm,
    );
    ep_offline.status = EndpointStatus::Offline;
    ep_offline.latency_ms = Some(100);

    let mut ep_pending = Endpoint::new(
        "Pending".to_string(),
        "http://localhost:8003".to_string(),
        EndpointType::Xllm,
    );
    ep_pending.status = EndpointStatus::Pending;
    ep_pending.latency_ms = Some(50);

    let endpoints = vec![ep_online, ep_offline, ep_pending];
    // Only the online endpoint with latency should be counted
    let result = fallback_avg_response_time(None, &endpoints);
    assert_eq!(result, Some(300.0));
}

#[test]
fn test_avg_response_time_fallback_single_online_endpoint() {
    let mut ep = Endpoint::new(
        "Single".to_string(),
        "http://localhost:8001".to_string(),
        EndpointType::Vllm,
    );
    ep.status = EndpointStatus::Online;
    ep.latency_ms = Some(42);

    let result = fallback_avg_response_time(None, &[ep]);
    assert_eq!(result, Some(42.0));
}

#[test]
fn test_avg_response_time_fallback_empty_endpoints() {
    let result = fallback_avg_response_time(None, &[]);
    assert_eq!(result, None);
}

#[test]
fn test_calculate_output_tps_weighted_by_generated_tokens() {
    use super::calculate_output_tps;
    use crate::balancer::EndpointTpsSummary;

    let result = calculate_output_tps(&[
        EndpointTpsSummary {
            endpoint_id: uuid::Uuid::nil(),
            model_count: 1,
            aggregate_tps: Some(50.0),
            total_output_tokens: 100,
            total_requests: 2,
        },
        EndpointTpsSummary {
            endpoint_id: uuid::Uuid::nil(),
            model_count: 1,
            aggregate_tps: Some(100.0),
            total_output_tokens: 100,
            total_requests: 2,
        },
    ])
    .expect("TPS should be computed from measured entries");

    assert!((result - 66.666_666_666_7).abs() < 0.000_001);
}

#[test]
fn test_calculate_output_tps_ignores_unmeasured_entries() {
    use super::calculate_output_tps;
    use crate::balancer::EndpointTpsSummary;

    let result = calculate_output_tps(&[
        EndpointTpsSummary {
            endpoint_id: uuid::Uuid::nil(),
            model_count: 0,
            aggregate_tps: None,
            total_output_tokens: 100,
            total_requests: 1,
        },
        EndpointTpsSummary {
            endpoint_id: uuid::Uuid::nil(),
            model_count: 1,
            aggregate_tps: Some(0.0),
            total_output_tokens: 100,
            total_requests: 1,
        },
    ]);

    assert_eq!(result, None);
}

#[test]
fn test_count_unique_model_ids_deduplicates_across_endpoints() {
    let result = count_unique_model_ids(vec![
        "llama-3.2:3b".to_string(),
        "qwen2.5:7b".to_string(),
        "llama-3.2:3b".to_string(),
    ]);

    assert_eq!(result, 2);
}

#[test]
fn test_action_items_count_offline_and_error_endpoints_independently() {
    let operations = DashboardOperations {
        health: "attention".to_string(),
        total_endpoints: 4,
        online_endpoints: 1,
        pending_endpoints: 0,
        registering_endpoints: 0,
        offline_endpoints: 2,
        error_endpoints: 1,
        total_requests: 0,
        successful_requests: 0,
        failed_requests: 0,
        success_rate: None,
        active_requests: 0,
        queued_requests: 0,
        average_response_time_ms: None,
        output_tps: None,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_tokens: 0,
        last_registered_at: None,
        last_seen_at: None,
    };

    let items = collect_action_items(&operations);
    let offline = items
        .iter()
        .find(|item| item.title == "Offline endpoints")
        .expect("offline endpoints action item should exist");
    let errors = items
        .iter()
        .find(|item| item.title == "Endpoint errors")
        .expect("endpoint errors action item should exist");

    assert_eq!(offline.count, 2);
    assert_eq!(errors.count, 1);
}

// --- DashboardOverview tests ---

#[test]
fn test_dashboard_overview_serialization() {
    use super::{DashboardActionItem, DashboardCapacity, DashboardOperations, DashboardOverview};

    let overview = DashboardOverview {
        endpoints: vec![],
        operations: DashboardOperations {
            health: "healthy".to_string(),
            total_endpoints: 0,
            online_endpoints: 0,
            pending_endpoints: 0,
            registering_endpoints: 0,
            offline_endpoints: 0,
            error_endpoints: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            success_rate: None,
            active_requests: 0,
            queued_requests: 0,
            average_response_time_ms: None,
            output_tps: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            last_registered_at: None,
            last_seen_at: None,
        },
        capacity: DashboardCapacity {
            total_models: 0,
            gpu_capable_endpoints: 0,
            gpu_telemetry_endpoints: 0,
            total_gpu_memory_bytes: None,
            used_gpu_memory_bytes: None,
            gpu_memory_usage_percent: None,
            telemetry_status: "unavailable".to_string(),
        },
        action_items: vec![DashboardActionItem {
            severity: "info".to_string(),
            title: "No action required".to_string(),
            detail: "All operational signals are nominal.".to_string(),
            count: 0,
        }],
        history: vec![],
        endpoint_tps: vec![],
        generated_at: chrono::Utc::now(),
        generation_time_ms: 42,
    };

    let json = serde_json::to_value(&overview).unwrap();
    assert!(json["endpoints"].is_array());
    assert!(json["operations"].is_object());
    assert!(json["capacity"].is_object());
    assert!(json["action_items"].is_array());
    assert!(json["stats"].is_null());
    assert!(json["operations"]["health"].is_string());
    assert!(json["operations"]["total_endpoints"].is_number());
    assert!(json["operations"]["output_tps"].is_null());
    assert!(json["capacity"]["gpu_capable_endpoints"].is_number());
    assert!(json["history"].is_array());
    assert!(json["endpoint_tps"].is_array());
    assert!(json["generated_at"].is_string());
    assert_eq!(json["generation_time_ms"], 42);
}

#[test]
fn test_dashboard_overview_equality() {
    use super::{DashboardActionItem, DashboardCapacity, DashboardOperations, DashboardOverview};

    let now = chrono::Utc::now();

    let overview = DashboardOverview {
        endpoints: vec![],
        operations: DashboardOperations {
            health: "healthy".to_string(),
            total_endpoints: 0,
            online_endpoints: 0,
            pending_endpoints: 0,
            registering_endpoints: 0,
            offline_endpoints: 0,
            error_endpoints: 0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            success_rate: None,
            active_requests: 0,
            queued_requests: 0,
            average_response_time_ms: None,
            output_tps: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            last_registered_at: None,
            last_seen_at: None,
        },
        capacity: DashboardCapacity {
            total_models: 0,
            gpu_capable_endpoints: 0,
            gpu_telemetry_endpoints: 0,
            total_gpu_memory_bytes: None,
            used_gpu_memory_bytes: None,
            gpu_memory_usage_percent: None,
            telemetry_status: "unavailable".to_string(),
        },
        action_items: vec![DashboardActionItem {
            severity: "info".to_string(),
            title: "No action required".to_string(),
            detail: "All operational signals are nominal.".to_string(),
            count: 0,
        }],
        history: vec![],
        endpoint_tps: vec![],
        generated_at: now,
        generation_time_ms: 0,
    };
    let overview2 = overview.clone();
    assert_eq!(overview, overview2);
}

// --- ModelTpsEntry extended tests ---

#[test]
fn test_model_tps_entry_with_none_tps() {
    use super::ModelTpsEntry;
    use crate::common::protocol::{TpsApiKind, TpsSource};

    let entry = ModelTpsEntry {
        model_id: "new-model".to_string(),
        api_kind: TpsApiKind::Completions,
        source: TpsSource::Benchmark,
        tps: None,
        request_count: 0,
        total_output_tokens: 0,
        average_duration_ms: None,
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert!(json["tps"].is_null());
    assert!(json["average_duration_ms"].is_null());
    assert_eq!(json["request_count"], 0);
}

#[test]
fn test_model_tps_entry_clone() {
    use super::ModelTpsEntry;
    use crate::common::protocol::{TpsApiKind, TpsSource};

    let entry = ModelTpsEntry {
        model_id: "model".to_string(),
        api_kind: TpsApiKind::ChatCompletions,
        source: TpsSource::Production,
        tps: Some(10.0),
        request_count: 5,
        total_output_tokens: 100,
        average_duration_ms: Some(50.0),
    };
    let cloned = entry.clone();
    assert_eq!(cloned.model_id, "model");
    assert_eq!(cloned.tps, Some(10.0));
}
