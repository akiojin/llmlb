//! Integration Test: Node Registration Flow
//!
//! ノードのCoordinatorへの登録フローをテスト

use ollama_router_common::{
    protocol::{RegisterRequest, RegisterStatus},
    types::GpuDeviceInfo,
};
use or_node::client::CoordinatorClient;
use std::net::IpAddr;

#[tokio::test]
#[ignore] // Coordinatorサーバーが必要
async fn test_agent_registration_to_coordinator() {
    // Arrange: Coordinatorが起動していると仮定
    let router_url =
        std::env::var("TEST_ROUTER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let mut client = CoordinatorClient::new(router_url);

    let register_req = RegisterRequest {
        machine_name: "test-agent-machine".to_string(),
        ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
        ollama_version: "0.1.0".to_string(),
        ollama_port: 11434,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
    };

    // Act: ノード登録
    let register_result = client.register(register_req.clone()).await;

    // Assert: 登録成功
    assert!(
        register_result.is_ok(),
        "Registration should succeed: {:?}",
        register_result
    );

    let response = register_result.unwrap();
    assert!(
        response.status == RegisterStatus::Registered || response.status == RegisterStatus::Updated,
        "Status should be Registered or Updated"
    );
    assert!(
        client.get_node_id().is_some(),
        "Node ID should be saved after registration"
    );

    println!("Registered agent ID: {}", response.node_id);
}

#[tokio::test]
#[ignore] // Coordinatorサーバーが必要
async fn test_agent_re_registration() {
    // Arrange: 同じノードを2回登録
    let router_url =
        std::env::var("TEST_ROUTER_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let mut client = CoordinatorClient::new(router_url);

    let register_req = RegisterRequest {
        machine_name: "test-re-register-machine".to_string(),
        ip_address: "192.168.1.101".parse().unwrap(),
        ollama_version: "0.1.0".to_string(),
        ollama_port: 11434,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
    };

    // Act: 初回登録
    let first_response = client.register(register_req.clone()).await.unwrap();
    assert_eq!(first_response.status, RegisterStatus::Registered);

    // Act: 再登録
    let second_response = client.register(register_req).await.unwrap();

    // Assert: 2回目は Updated ステータス
    assert_eq!(
        second_response.status,
        RegisterStatus::Updated,
        "Second registration should return Updated status"
    );

    // Assert: 同じNode IDが返される
    assert_eq!(
        first_response.node_id, second_response.node_id,
        "Node ID should remain the same on re-registration"
    );
}

#[tokio::test]
async fn test_agent_registration_invalid_coordinator() {
    // Arrange: 存在しないCoordinatorURL
    let mut client = CoordinatorClient::new("http://invalid-coordinator-url:9999".to_string());

    let register_req = RegisterRequest {
        machine_name: "test-machine".to_string(),
        ip_address: "192.168.1.100".parse().unwrap(),
        ollama_version: "0.1.0".to_string(),
        ollama_port: 11434,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: None,
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
    };

    // Act: 登録試行
    let result = client.register(register_req).await;

    // Assert: 接続エラーが発生すること
    assert!(result.is_err(), "Should fail with connection error");
    println!("Expected error: {:?}", result.unwrap_err());
}

#[test]
fn test_node_id_persistence() {
    // Arrange
    let client = CoordinatorClient::new("http://localhost:8080".to_string());

    // Act & Assert: 初期状態ではNode IDがない
    assert!(
        client.get_node_id().is_none(),
        "Node ID should be None before registration"
    );

    // Note: 実際の登録はasync testで行うため、ここでは状態のみ確認
}
