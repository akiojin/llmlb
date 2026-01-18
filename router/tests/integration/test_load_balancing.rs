#![allow(deprecated)] // NodeRegistry → EndpointRegistry migration in progress

//! Integration Test: ロードバランシング
//!
//! 複数ノードへのリクエスト分散と負荷ベース選択の検証

use llm_router::{
    balancer::{LoadManager, MetricsUpdate, RequestOutcome},
    registry::NodeRegistry,
};
use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

#[tokio::test]
#[ignore = "Registering→Online state transition needs investigation with round-robin distribution"]
async fn test_round_robin_load_balancing() {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());

    // 全ノードを先に登録
    let mut node_ids = Vec::new();
    for idx in 0..3 {
        let req = RegisterRequest {
            machine_name: format!("round-robin-node-{}", idx),
            ip_address: format!("192.168.1.{}", 200 + idx)
                .parse::<IpAddr>()
                .unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        };
        let response = registry.register(req).await.unwrap();
        node_ids.push(response.node_id);
    }

    for node_id in &node_ids {
        registry.approve(*node_id).await.unwrap();
    }

    // 登録後に全ノードにメトリクスを記録してOnlineに遷移
    // 全ノードに同じaverage_response_time_msを設定してラウンドロビン均等化
    for node_id in &node_ids {
        load_manager
            .record_metrics(MetricsUpdate {
                node_id: *node_id,
                cpu_usage: 20.0,
                memory_usage: 30.0,
                gpu_usage: None,
                gpu_memory_usage: None,
                gpu_memory_total_mb: None,
                gpu_memory_used_mb: None,
                gpu_temperature: None,
                gpu_model_name: None,
                gpu_compute_capability: None,
                gpu_capability_score: None,
                active_requests: 0,
                average_response_time_ms: Some(100.0),
                initializing: false,
                ready_models: Some((0, 0)),
            })
            .await
            .unwrap();
    }

    let mut distribution: HashMap<_, usize> = HashMap::new();

    for _ in 0..9 {
        let node = load_manager.select_node().await.unwrap();
        let entry = distribution.entry(node.id).or_default();
        *entry += 1;

        load_manager.begin_request(node.id).await.unwrap();
        load_manager
            .finish_request(node.id, RequestOutcome::Success, Duration::from_millis(50))
            .await
            .unwrap();
    }

    // 各ノードに少なくとも1回はリクエストが分配されることを確認
    // （正確な3-3-3分配はunit testでカバー済み）
    for node_id in &node_ids {
        let count = distribution.get(node_id).copied().unwrap_or_default();
        assert!(
            count >= 1,
            "Each online node should receive at least 1 request, but node {} got {}",
            node_id,
            count
        );
    }

    // 全ノードが選択対象になっていることを確認
    assert_eq!(
        distribution.len(),
        node_ids.len(),
        "All {} nodes should be selected at least once",
        node_ids.len()
    );
}

#[tokio::test]
async fn test_load_based_balancing_favors_low_cpu_nodes() {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());

    let high_cpu_node = registry
        .register(RegisterRequest {
            machine_name: "high-cpu-node".to_string(),
            ip_address: "192.168.2.10".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;

    let low_cpu_node = registry
        .register(RegisterRequest {
            machine_name: "low-cpu-node".to_string(),
            ip_address: "192.168.2.11".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;
    registry.approve(high_cpu_node).await.unwrap();
    registry.approve(low_cpu_node).await.unwrap();

    // 高負荷ノードはCPU 95%、低負荷ノードはCPU 10%
    load_manager
        .record_metrics(MetricsUpdate {
            node_id: high_cpu_node,
            cpu_usage: 95.0,
            memory_usage: 40.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 2,
            average_response_time_ms: None,
            initializing: false,
            ready_models: Some((0, 0)),
        })
        .await
        .unwrap();
    load_manager
        .record_metrics(MetricsUpdate {
            node_id: low_cpu_node,
            cpu_usage: 10.0,
            memory_usage: 30.0,
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
            ready_models: Some((0, 0)),
        })
        .await
        .unwrap();

    for _ in 0..10 {
        let selected = load_manager.select_node().await.unwrap();
        assert_eq!(
            selected.id, low_cpu_node,
            "Load-based balancer should prefer low CPU node"
        );

        load_manager.begin_request(selected.id).await.unwrap();
        load_manager
            .finish_request(
                selected.id,
                RequestOutcome::Success,
                Duration::from_millis(25),
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_load_based_balancing_prefers_lower_latency() {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());

    let slow_node = registry
        .register(RegisterRequest {
            machine_name: "slow-node".to_string(),
            ip_address: "192.168.3.10".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;

    let fast_node = registry
        .register(RegisterRequest {
            machine_name: "fast-node".to_string(),
            ip_address: "192.168.3.11".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;
    registry.approve(slow_node).await.unwrap();
    registry.approve(fast_node).await.unwrap();

    load_manager
        .record_metrics(MetricsUpdate {
            node_id: slow_node,
            cpu_usage: 50.0,
            memory_usage: 40.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 1,
            average_response_time_ms: Some(250.0),
            initializing: false,
            ready_models: Some((0, 0)),
        })
        .await
        .unwrap();
    load_manager
        .record_metrics(MetricsUpdate {
            node_id: fast_node,
            cpu_usage: 50.0,
            memory_usage: 40.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 1,
            average_response_time_ms: Some(120.0),
            initializing: false,
            ready_models: Some((0, 0)),
        })
        .await
        .unwrap();

    let selected = load_manager.select_node().await.unwrap();
    assert_eq!(selected.id, fast_node);
}

#[tokio::test]
async fn test_load_balancer_excludes_non_online_nodes() {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());

    // Pending 状態のノード（承認されるまで Online にならない）
    let pending_node = registry
        .register(RegisterRequest {
            machine_name: "pending-node".to_string(),
            ip_address: "192.168.4.10".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;

    // Online 状態のノード（承認後にメトリクスを記録して Online に遷移）
    let online_node = registry
        .register(RegisterRequest {
            machine_name: "online-node".to_string(),
            ip_address: "192.168.4.11".parse::<IpAddr>().unwrap(),
            runtime_version: "0.1.0".to_string(),
            runtime_port: 32768,
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap()
        .node_id;
    registry.approve(online_node).await.unwrap();

    // online_agent のみメトリクスを記録して Online に遷移
    load_manager
        .record_metrics(MetricsUpdate {
            node_id: online_node,
            cpu_usage: 50.0,
            memory_usage: 40.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            average_response_time_ms: Some(100.0),
            initializing: false,
            ready_models: Some((0, 0)),
        })
        .await
        .unwrap();

    // select_node は Online のノードのみを選択すべき
    for _ in 0..5 {
        let selected = load_manager.select_node().await.unwrap();
        assert_eq!(
            selected.id, online_node,
            "Load balancer should exclude non-online nodes and only select Online nodes"
        );
        assert_ne!(
            selected.id, pending_node,
            "Pending node should not be selected"
        );

        load_manager.begin_request(selected.id).await.unwrap();
        load_manager
            .finish_request(
                selected.id,
                RequestOutcome::Success,
                Duration::from_millis(50),
            )
            .await
            .unwrap();
    }
}
