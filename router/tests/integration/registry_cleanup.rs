//! Integration Test: GPUなしノードの起動時クリーンアップ
//!
//! ストレージに保存されたGPU無しノードが、Router起動時に自動削除されることを確認する。

use chrono::Utc;
use llm_router::db::nodes::NodeStorage;
use llm_router::registry::NodeRegistry;
use llm_router_common::types::{GpuDeviceInfo, Node, NodeStatus};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::IpAddr;
use uuid::Uuid;

/// テスト用のインメモリSQLiteプールを作成
async fn create_test_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory pool");

    // マイグレーションを実行
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// GPU有効なノードを作成
fn create_gpu_node(machine_name: &str) -> Node {
    let now = Utc::now();
    Node {
        id: Uuid::new_v4(),
        machine_name: machine_name.to_string(),
        ip_address: "192.168.1.100".parse::<IpAddr>().unwrap(),
        runtime_version: "0.1.0".to_string(),
        runtime_port: 32768,
        status: NodeStatus::Online,
        registered_at: now,
        last_seen: now,
        online_since: Some(now),
        custom_name: None,
        tags: Vec::new(),
        notes: None,
        loaded_models: Vec::new(),
        loaded_embedding_models: Vec::new(),
        loaded_asr_models: Vec::new(),
        loaded_tts_models: Vec::new(),
        supported_runtimes: Vec::new(),
        gpu_devices: vec![GpuDeviceInfo {
            model: "NVIDIA RTX 4090".to_string(),
            count: 1,
            memory: Some(24_000_000_000),
        }],
        gpu_available: true,
        gpu_count: Some(1),
        gpu_model: Some("NVIDIA RTX 4090".to_string()),
        gpu_model_name: Some("GeForce RTX 4090".to_string()),
        gpu_compute_capability: Some("8.9".to_string()),
        gpu_capability_score: Some(89),
        node_api_port: Some(32769),
        initializing: false,
        ready_models: None,
        sync_state: None,
        sync_progress: None,
        sync_updated_at: None,
        executable_models: Vec::new(),
        excluded_models: Vec::new(),
    }
}

/// GPU無効なノードを作成
fn create_no_gpu_node(machine_name: &str) -> Node {
    let now = Utc::now();
    Node {
        id: Uuid::new_v4(),
        machine_name: machine_name.to_string(),
        ip_address: "192.168.1.101".parse::<IpAddr>().unwrap(),
        runtime_version: "0.1.0".to_string(),
        runtime_port: 32768,
        status: NodeStatus::Online,
        registered_at: now,
        last_seen: now,
        online_since: Some(now),
        custom_name: None,
        tags: Vec::new(),
        notes: None,
        loaded_models: Vec::new(),
        loaded_embedding_models: Vec::new(),
        loaded_asr_models: Vec::new(),
        loaded_tts_models: Vec::new(),
        supported_runtimes: Vec::new(),
        gpu_devices: Vec::new(),
        gpu_available: false,
        gpu_count: None,
        gpu_model: None,
        gpu_model_name: None,
        gpu_compute_capability: None,
        gpu_capability_score: None,
        node_api_port: Some(32769),
        initializing: false,
        ready_models: None,
        sync_state: None,
        sync_progress: None,
        sync_updated_at: None,
        executable_models: Vec::new(),
        excluded_models: Vec::new(),
    }
}

#[tokio::test]
async fn gpu_less_nodes_are_removed_on_startup() {
    let pool = create_test_pool().await;

    // 準備: GPU有り・GPU無しのノードをデータベースに直接挿入
    let storage = NodeStorage::new(pool.clone());

    let gpu_node1 = create_gpu_node("gpu-node-1");
    let gpu_node2 = create_gpu_node("gpu-node-2");
    let no_gpu_node1 = create_no_gpu_node("no-gpu-node-1");
    let no_gpu_node2 = create_no_gpu_node("no-gpu-node-2");

    storage.save_node(&gpu_node1).await.unwrap();
    storage.save_node(&gpu_node2).await.unwrap();
    storage.save_node(&no_gpu_node1).await.unwrap();
    storage.save_node(&no_gpu_node2).await.unwrap();

    // 4ノードが保存されていることを確認
    let all_nodes = storage.load_nodes().await.unwrap();
    assert_eq!(all_nodes.len(), 4);

    // Act: ストレージ付きレジストリを初期化（クリーンアップが実行される）
    let registry = NodeRegistry::with_storage(pool.clone())
        .await
        .expect("registry should initialize");

    // Assert: レジストリにはGPUありノードのみ残る
    let remaining = registry.list().await;
    assert_eq!(
        remaining.len(),
        2,
        "only GPU-capable nodes should remain after cleanup"
    );
    assert!(remaining.iter().all(|node| node.gpu_available));

    // データベースからもGPU無しノードが削除されていることを確認
    let persisted = storage.load_nodes().await.unwrap();
    assert_eq!(persisted.len(), 2);
    assert!(persisted.iter().all(|node| node.gpu_available));
    assert!(persisted.iter().all(|node| !node.gpu_devices.is_empty()));
}
