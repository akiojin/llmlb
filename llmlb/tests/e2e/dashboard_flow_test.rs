//! ダッシュボードフローE2Eテスト
//!
//! ダッシュボードAPI（/api/dashboard/*）のE2Eテスト
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::{ApiKeyScope, UserRole};
use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool, String) {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLMLB_DATA_DIR", &temp_dir);

    let db_pool = support::lb::create_test_db_pool().await;
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = support::lb::test_jwt_secret();

    let state = AppState {
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let admin_user = llmlb::db::users::create(&db_pool, "admin", &password_hash, UserRole::Admin)
        .await
        .expect("create admin user");
    let admin_key = llmlb::db::api_keys::create(
        &db_pool,
        "admin-key",
        admin_user.id,
        None,
        vec![ApiKeyScope::Admin],
    )
    .await
    .expect("create admin api key")
    .key;

    (api::create_app(state), db_pool, admin_key)
}

#[tokio::test]
async fn test_dashboard_stats_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /api/dashboard/stats
    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/dashboard/stats")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/dashboard/stats should return OK"
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

    // GET /api/dashboard/overview
    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/dashboard/overview")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/dashboard/overview should return OK"
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

    // GET /api/dashboard/request-history
    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/dashboard/request-history")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/dashboard/request-history should return OK"
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
async fn test_dashboard_endpoints_include_endpoint_type() {
    let (app, _db_pool, admin_key) = build_app().await;

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    let create_body = json!({
        "name": "Test vLLM",
        "base_url": mock.uri(),
        "endpoint_type": "vllm"
    });

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("POST")
                .uri("/api/endpoints")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        create_response.status(),
        StatusCode::CREATED,
        "POST /api/endpoints should return CREATED"
    );

    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/dashboard/endpoints")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/dashboard/endpoints should return OK"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let endpoints: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let endpoints = endpoints.as_array().expect("response should be an array");
    assert!(
        endpoints
            .iter()
            .any(|endpoint| endpoint["endpoint_type"] == "vllm"),
        "endpoint_type should be included in dashboard endpoints"
    );
}

#[tokio::test]
async fn test_cloud_metrics_endpoint() {
    let (app, _db_pool, admin_key) = build_app().await;

    // GET /api/metrics/cloud
    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/metrics/cloud")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET /api/metrics/cloud should return OK"
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

    // GET /api/models/loaded
    let response = app
        .oneshot(
            Request::builder()
                .header("x-internal-token", "test-internal")
                .method("GET")
                .uri("/api/models/loaded")
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
        "/api/models/loaded should be removed (got {})",
        response.status()
    );
}
