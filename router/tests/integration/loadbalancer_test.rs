#![allow(deprecated)] // NodeRegistry → EndpointRegistry migration in progress

//! Integration Test: 負荷ベースロードバランシング
//!
//! ⚠️ このテストはTDD RED状態の統合テストです。
//! メトリクスベースのロードバランシングはSPEC-589f2df1で実装済みであり、
//! balancer::testsで十分にカバーされています。

#[tokio::test]
#[ignore = "TDD RED phase - load balancing implemented in SPEC-589f2df1, covered by unit tests"]
async fn test_select_node_by_metrics_prefers_low_load() {
    // Arrange: 3台のノード（1台が高負荷 CPU 90%、他は低負荷）
    // let registry = llm_router::registry::NodeRegistry::new();
    // let load_manager = llm_router::balancer::LoadManager::new(registry.clone());

    // ノード登録
    // let node1_id = registry.register(create_test_node_request("node1", "192.168.10.1")).await.unwrap().node_id;
    // let node2_id = registry.register(create_test_node_request("node2", "192.168.10.2")).await.unwrap().node_id;
    // let node3_id = registry.register(create_test_node_request("node3", "192.168.10.3")).await.unwrap().node_id;

    // メトリクス登録（node1は高負荷、node2とnode3は低負荷）
    // load_manager.record_metrics(create_metrics(node1_id, 90.0, 80.0, 5)).await.unwrap(); // 高負荷
    // load_manager.record_metrics(create_metrics(node2_id, 20.0, 30.0, 1)).await.unwrap(); // 低負荷
    // load_manager.record_metrics(create_metrics(node3_id, 25.0, 35.0, 1)).await.unwrap(); // 低負荷

    // Act: select_node_by_metrics() を呼び出し
    // let selected = load_manager.select_node_by_metrics().await.unwrap();

    // Assert: 高負荷のnode1は選択されず、低負荷のnode2またはnode3が選択される
    // assert!(
    //     selected.id == node2_id || selected.id == node3_id,
    //     "Should select low-load node (node2 or node3), but selected: {:?}",
    //     selected.id
    // );
    // assert_ne!(selected.id, node1_id, "Should NOT select high-load node1");

    // TODO: T014-T015で実装後にアンコメント
    panic!("RED: select_node_by_metrics()が未実装");
}

#[tokio::test]
#[ignore = "TDD RED phase - load balancing implemented in SPEC-589f2df1, covered by unit tests"]
async fn test_fallback_to_round_robin_when_all_nodes_high_load() {
    // Arrange: 3台のノード（すべてがCPU 95%の高負荷）
    // let registry = llm_router::registry::NodeRegistry::new();
    // let load_manager = llm_router::balancer::LoadManager::new(registry.clone());

    // ノード登録
    // let node1_id = registry.register(create_test_node_request("node1", "192.168.11.1")).await.unwrap().node_id;
    // let node2_id = registry.register(create_test_node_request("node2", "192.168.11.2")).await.unwrap().node_id;
    // let node3_id = registry.register(create_test_node_request("node3", "192.168.11.3")).await.unwrap().node_id;

    // メトリクス登録（すべて高負荷 CPU > 80%）
    // load_manager.record_metrics(create_metrics(node1_id, 95.0, 90.0, 8)).await.unwrap();
    // load_manager.record_metrics(create_metrics(node2_id, 96.0, 91.0, 9)).await.unwrap();
    // load_manager.record_metrics(create_metrics(node3_id, 97.0, 92.0, 10)).await.unwrap();

    // Act: select_node_by_metrics() を複数回呼び出し（ラウンドロビンになるか検証）
    // let mut distribution = std::collections::HashMap::new();
    // for _ in 0..9 {
    //     let selected = load_manager.select_node_by_metrics().await.unwrap();
    //     *distribution.entry(selected.id).or_insert(0) += 1;
    // }

    // Assert: すべてのノードが高負荷のため、ラウンドロビンにフォールバック
    // 各ノードが均等に選択される（9回 ÷ 3台 = 3回ずつ）
    // assert_eq!(distribution.get(&node1_id).copied().unwrap_or(0), 3, "node1 should be selected 3 times (round-robin)");
    // assert_eq!(distribution.get(&node2_id).copied().unwrap_or(0), 3, "node2 should be selected 3 times (round-robin)");
    // assert_eq!(distribution.get(&node3_id).copied().unwrap_or(0), 3, "node3 should be selected 3 times (round-robin)");

    // TODO: T014-T015で実装後にアンコメント
    panic!("RED: 全ノード高負荷時のラウンドロビンフォールバックが未実装");
}

// ヘルパー関数（実装時に使用）
// fn create_test_node_request(name: &str, ip: &str) -> RegisterRequest {
//     use llm_router_common::{protocol::RegisterRequest, types::GpuDeviceInfo};
//     RegisterRequest {
//         machine_name: name.to_string(),
//         ip_address: ip.parse().unwrap(),
//         runtime_version: "0.1.0".to_string(),
//         runtime_port: 32768,
//         gpu_available: true,
//         gpu_devices: vec![GpuDeviceInfo {
//             model: "Test GPU".to_string(),
//             count: 1,
//             memory: None,
//         }],
//         gpu_count: Some(1),
//         gpu_model: Some("Test GPU".to_string()),
//     }
// }

// fn create_metrics(node_id: Uuid, cpu: f64, memory: f64, active_reqs: u32) -> MetricsUpdate {
//     use llm_router::balancer::MetricsUpdate;
//     MetricsUpdate {
//         node_id,
//         cpu_usage: cpu,
//         memory_usage: memory,
//         gpu_usage: None,
//         gpu_memory_usage: None,
//         gpu_memory_total_mb: None,
//         gpu_memory_used_mb: None,
//         gpu_temperature: None,
//         gpu_model_name: None,
//         gpu_compute_capability: None,
//         gpu_capability_score: None,
//         active_requests: active_reqs,
//         average_response_time_ms: None,
//     }
// }
