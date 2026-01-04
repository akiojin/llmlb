//! ノードフローE2Eテスト
//!
//! T093: 完全なノードフロー（登録 → トークン使用 → ヘルスチェック）

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::{
    auth::{ApiKeyScope, UserRole},
    protocol::RegisterRequest,
    types::GpuDeviceInfo,
};
use serde_json::{json, Value};
use std::net::IpAddr;
use tower::ServiceExt;
use uuid::Uuid;

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool, String) {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = support::router::create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = support::router::test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llm_router::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llm_router::db::api_keys::create(
        &db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_router(state), db_pool, admin_key)
}

async fn login_admin(app: &Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "username": "admin",
                        "password": "password123"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Admin login should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let data: Value = serde_json::from_slice(&body).unwrap();
    data["token"].as_str().unwrap().to_string()
}

async fn approve_node(app: &Router, node_id: Uuid) -> Value {
    let token = login_admin(app).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v0/nodes/{}/approve", node_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Approve should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn fetch_node_status(app: &Router, node_id: Uuid, admin_key: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/nodes should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: Value = serde_json::from_slice(&body).unwrap();
    let node_id = node_id.to_string();
    let status = nodes
        .as_array()
        .and_then(|items| {
            items.iter().find(|node| {
                node.get("id")
                    .and_then(|id| id.as_str())
                    .map(|id| id == node_id)
                    .unwrap_or(false)
            })
        })
        .and_then(|node| node.get("status").and_then(|status| status.as_str()))
        .unwrap();

    status.to_string()
}

#[tokio::test]
async fn test_complete_node_flow() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // Step 1: ノード登録（モックサーバーのポートを使用）
    let register_request = RegisterRequest {
        machine_name: "test-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.0".to_string(),
        runtime_port: mock_node.runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: Some(8192),
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = register_response.status();
    let register_body = axum::body::to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();

    if status != StatusCode::CREATED {
        let error_text = String::from_utf8_lossy(&register_body);
        eprintln!("Registration failed with status: {:?}", status);
        eprintln!("Error body: {}", error_text);
    }
    assert_eq!(status, StatusCode::CREATED);

    let register_data: serde_json::Value = serde_json::from_slice(&register_body).unwrap();

    let node_id = Uuid::parse_str(register_data["node_id"].as_str().unwrap()).unwrap();
    let node_token = register_data["node_token"].as_str().unwrap();

    assert!(!node_token.is_empty(), "Node token should be returned");

    // Step 2: トークンを使ってヘルスチェックを送信
    let heartbeat_request = json!({
        "node_id": node_id.to_string(),
        "cpu_usage": 50.0,
        "memory_usage": 60.0,
        "gpu_usage": 40.0,
        "gpu_memory_usage": 50.0,
        "gpu_memory_total_mb": 8192,
        "gpu_memory_used_mb": 4096,
        "gpu_temperature": 65.0,
        "active_requests": 0,
        "loaded_models": [],
        "initializing": false,
        "ready_models": [1, 1]
    });

    let heartbeat_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&heartbeat_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        heartbeat_response.status(),
        StatusCode::OK,
        "Heartbeat with valid token should succeed"
    );

    let pending_status = fetch_node_status(&app, node_id, &admin_key).await;
    assert_eq!(
        pending_status, "pending",
        "Pending node should stay pending even after heartbeat"
    );

    let approved_node = approve_node(&app, node_id).await;
    assert_eq!(
        approved_node["status"].as_str(),
        Some("online"),
        "Approved node should become online when ready"
    );

    // Step 3: APIキーなしでヘルスチェックを送信 → 失敗
    let unauthorized_heartbeat_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&heartbeat_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        unauthorized_heartbeat_response.status(),
        StatusCode::UNAUTHORIZED,
        "Heartbeat without API key should fail"
    );

    // Step 4: トークンなしでヘルスチェックを送信 → 失敗
    let missing_token_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::from(serde_json::to_vec(&heartbeat_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        missing_token_response.status(),
        StatusCode::UNAUTHORIZED,
        "Heartbeat without token should fail"
    );

    // Step 5: 無効なトークンでヘルスチェックを送信 → 失敗
    let invalid_token_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("x-node-token", "invalid-token-12345")
                .body(Body::from(serde_json::to_vec(&heartbeat_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        invalid_token_response.status(),
        StatusCode::UNAUTHORIZED,
        "Heartbeat with invalid token should fail"
    );
}

#[tokio::test]
async fn test_node_token_persistence() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // ノード登録
    let register_request = RegisterRequest {
        machine_name: "test-node-2".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.0".to_string(),
        runtime_port: mock_node.runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: Some(8192),
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
        supported_runtimes: Vec::new(),
    };

    let first_register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first_register_response.status(), StatusCode::CREATED);

    let first_body = axum::body::to_bytes(first_register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let first_data: serde_json::Value = serde_json::from_slice(&first_body).unwrap();

    let node_id = first_data["node_id"].as_str().unwrap();
    let first_token = first_data["node_token"].as_str().unwrap();

    assert!(
        !first_token.is_empty(),
        "First registration should return token"
    );

    let node_id = Uuid::parse_str(node_id).unwrap();
    let approved_node = approve_node(&app, node_id).await;
    assert_eq!(
        approved_node["status"].as_str(),
        Some("online"),
        "Approved node should become online"
    );

    // 同じノードを再度登録（更新）
    let second_register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .header("x-node-token", first_token)
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        second_register_response.status(),
        StatusCode::OK,
        "Re-registration should return 200 OK (update)"
    );

    let second_body = axum::body::to_bytes(second_register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let second_data: serde_json::Value = serde_json::from_slice(&second_body).unwrap();

    let second_agent_id = second_data["node_id"].as_str().unwrap();

    // 同じノードIDが返される
    assert_eq!(
        node_id.to_string(),
        second_agent_id,
        "Re-registration should return same node ID"
    );

    // 2回目の登録でも新しいトークンが返される（プロトコル変更により、更新時もトークンを再生成）
    assert!(
        second_data["node_token"].is_string(),
        "Re-registration should return a new token"
    );

    let status = fetch_node_status(&app, node_id, &admin_key).await;
    assert_eq!(
        status, "pending",
        "Re-registration should reset status to pending"
    );
}

#[tokio::test]
async fn test_list_nodes() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // ノードを登録
    let register_request = RegisterRequest {
        machine_name: "list-test-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.0".to_string(),
        runtime_port: mock_node.runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: Some(8192),
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
        supported_runtimes: Vec::new(),
    };

    let _register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // GET /v0/nodes でノード一覧を取得
    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        list_response.status(),
        StatusCode::OK,
        "GET /v0/nodes should return OK"
    );

    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(nodes.is_array(), "Response should be an array");
    let nodes_array = nodes.as_array().unwrap();
    assert!(
        !nodes_array.is_empty(),
        "Should have at least one registered node"
    );

    // ノードの構造を検証
    let node = &nodes_array[0];
    assert!(node.get("id").is_some(), "Node must have 'id' field");
    assert!(
        node.get("machine_name").is_some(),
        "Node must have 'machine_name' field"
    );
}

#[tokio::test]
async fn test_node_metrics_update() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // ノードを登録
    let register_request = RegisterRequest {
        machine_name: "metrics-test-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.0".to_string(),
        runtime_port: mock_node.runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Test GPU".to_string(),
            count: 1,
            memory: Some(8192),
        }],
        gpu_count: Some(1),
        gpu_model: Some("Test GPU".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_data: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = register_data["node_id"].as_str().unwrap();
    let node_token = register_data["node_token"].as_str().unwrap();

    // POST /v0/health でヘルス/メトリクスを更新
    let metrics_request = json!({
        "node_id": node_id,
        "cpu_usage": 45.5,
        "memory_usage": 60.2,
        "active_requests": 3,
        "average_response_time_ms": 250.5,
        "loaded_models": ["gpt-oss:20b"],
        "loaded_embedding_models": [],
        "initializing": false,
        "ready_models": [1, 1]
    });

    let metrics_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&metrics_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        metrics_response.status() == StatusCode::OK,
        "POST /v0/health should return OK"
    );
}

#[tokio::test]
async fn test_list_node_metrics() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/nodes/metrics でメトリクス一覧を取得
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes/metrics")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/nodes/metrics should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // メトリクスはオブジェクトまたは配列
    assert!(
        metrics.is_object() || metrics.is_array(),
        "Response should be an object or array"
    );
}

#[tokio::test]
async fn test_metrics_summary() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/metrics/summary
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/metrics/summary")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/metrics/summary should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let summary: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(summary.is_object(), "Response should be an object");
}
