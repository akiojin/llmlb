//! ダッシュボードフローE2Eテスト
//!
//! ダッシュボードAPI（/v0/dashboard/*）のE2Eテスト

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
use std::net::IpAddr;
use tower::ServiceExt;

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool, String) {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = support::router::create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = support::router::test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
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
async fn test_dashboard_nodes_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/dashboard/nodes
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/dashboard/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/dashboard/nodes should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        nodes.is_array(),
        "Response should be an array of dashboard nodes"
    );
}

#[tokio::test]
async fn test_dashboard_stats_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/dashboard/stats
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/dashboard/stats")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/dashboard/stats should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(stats.is_object(), "Response should be a stats object");
}

#[tokio::test]
async fn test_dashboard_overview_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/dashboard/overview
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/dashboard/overview")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/dashboard/overview should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let overview: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        overview.is_object(),
        "Response should be an overview object"
    );
}

#[tokio::test]
async fn test_dashboard_request_history_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/dashboard/request-history
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/dashboard/request-history")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/dashboard/request-history should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let history: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        history.is_array(),
        "Response should be an array of request history"
    );
}

#[tokio::test]
async fn test_dashboard_nodes_with_registered_node() {
    // モックノードサーバーを起動
    let mock_node = support::node::MockNodeServer::start().await;
    let (app, _db_pool, admin_key) = build_app().await;

    // ノードを登録（モックサーバーのポートを使用）
    let register_request = RegisterRequest {
        machine_name: "dashboard-test-node".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.0".to_string(),
        runtime_port: mock_node.runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "RTX 4090".to_string(),
            count: 1,
            memory: Some(24576),
        }],
        gpu_count: Some(1),
        gpu_model: Some("RTX 4090".to_string()),
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

    // ダッシュボードノード一覧を取得
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/dashboard/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let nodes_array = nodes.as_array().unwrap();
    assert!(
        !nodes_array.is_empty(),
        "Dashboard should show registered nodes"
    );
}

#[tokio::test]
async fn test_cloud_metrics_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/metrics/cloud
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/metrics/cloud")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /v0/metrics/cloud should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();

    // Prometheus形式のメトリクスが含まれることを確認
    // メトリクスが空の場合もあるので、形式チェックのみ
    assert!(
        metrics_text.is_empty() || metrics_text.contains("# ") || metrics_text.contains("_"),
        "Response should be in Prometheus text format"
    );
}

#[tokio::test]
async fn test_models_loaded_endpoint_is_removed() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /v0/models/loaded
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/models/loaded")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "/v0/models/loaded should be removed (got {})",
        response.status()
    );
}
