#![allow(deprecated)] // NodeRegistry → EndpointRegistry migration in progress

//! Integration Test: ダッシュボードAPIでのGPU情報表示
//!
//! ダッシュボードエンドポイントがノードのGPU情報（モデル名・枚数）を返すことを検証する。
//!
//! SPEC-66555000: POST /v0/nodes は廃止され、/v0/internal/test/register-node に置き換えられました。

use axum::{
    body::{to_bytes, Body},
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
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn build_router() -> (Router, String) {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let endpoint_registry = llm_router::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    #[allow(deprecated)]
    let state = AppState {
        registry,
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };
    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llm_router::db::users::create(&state.db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llm_router::db::api_keys::create(
        &state.db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_router(state), admin_key)
}

#[tokio::test]
async fn dashboard_nodes_include_gpu_devices() {
    // モックサーバーを起動
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {"id": "test-model", "object": "model"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let mock_port = mock_server.address().port();
    let runtime_port = mock_port - 1;

    let (router, admin_key) = build_router().await;

    let register_request = RegisterRequest {
        machine_name: "dashboard-gpu".to_string(),
        ip_address: IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        runtime_version: "0.1.42".to_string(),
        runtime_port,
        gpu_available: true,
        gpu_devices: vec![GpuDeviceInfo {
            model: "Apple M3 Max".to_string(),
            count: 1,
            memory: Some(24576),
        }],
        gpu_count: Some(1),
        gpu_model: Some("Apple M3 Max".to_string()),
        supported_runtimes: Vec::new(),
    };

    let register_response = router
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

    assert_eq!(register_response.status(), StatusCode::CREATED);

    let response = router
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
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        payload.is_array(),
        "expected array response but got {payload:?}"
    );
    let node = payload
        .as_array()
        .and_then(|list| list.first())
        .cloned()
        .expect("node entry must exist");
    assert_eq!(node["machine_name"], "dashboard-gpu");
    assert!(
        node["gpu_devices"].is_array(),
        "gpu_devices should be present in dashboard payload"
    );
    let devices = node["gpu_devices"].as_array().unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["model"], "Apple M3 Max");
    assert_eq!(devices[0]["count"], 1);
}
