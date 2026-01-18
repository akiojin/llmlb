//! ノード管理E2Eテスト
//!
//! SPEC-66555000: POST /v0/nodes と POST /v0/health は廃止されました。
//! ノードトークン関連の機能はEndpoints APIに移行されています。
//! このテストファイルではGET系の管理APIのみをテストします。

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
use serde_json::Value;
use std::net::IpAddr;
use tower::ServiceExt;

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool, String) {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = support::router::create_test_db_pool().await;
    let endpoint_registry = llm_router::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = support::router::test_jwt_secret();

    #[allow(deprecated)]
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
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

#[tokio::test]
async fn test_list_nodes() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // ノードを登録
    // SPEC-66555000: POST /v0/nodes は廃止され、デバッグ用内部エンドポイントを使用
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
                .uri("/v0/internal/test/register-node")
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
    let summary: Value = serde_json::from_slice(&body).unwrap();

    assert!(summary.is_object(), "Response should be an object");
}
