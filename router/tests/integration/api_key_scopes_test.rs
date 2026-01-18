#![allow(deprecated)] // NodeRegistry → EndpointRegistry migration in progress

//! Integration Test: APIキーのスコープ制御
//!
//! /v0 と /v1 の各エンドポイントでスコープが正しく強制されることを確認する。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::json;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool) {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let db_pool = support::router::create_test_db_pool().await;
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

    (api::create_router(state), db_pool)
}

async fn create_admin_user(db_pool: &sqlx::SqlitePool) -> uuid::Uuid {
    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    let created = llm_router::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin)
        .await;
    if let Ok(user) = created {
        return user.id;
    }
    llm_router::db::users::find_by_username(db_pool, "admin")
        .await
        .unwrap()
        .unwrap()
        .id
}

async fn create_api_key(
    db_pool: &sqlx::SqlitePool,
    scopes: Vec<ApiKeyScope>,
) -> String {
    let admin_id = create_admin_user(db_pool).await;
    let api_key = llm_router::db::api_keys::create(db_pool, "test-key", admin_id, None, scopes)
        .await
        .expect("create api key");
    api_key.key
}

fn node_payload(runtime_port: u16) -> serde_json::Value {
    json!({
        "machine_name": "scope-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0-test",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
        ]
    })
}

#[tokio::test]
async fn v0_nodes_requires_node_register_scope() {
    let (app, db_pool) = build_app().await;

    let node_key =
        create_api_key(&db_pool, vec![ApiKeyScope::Node]).await;
    let api_key =
        create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;

    let mock_server = MockServer::start().await;
    // SPEC-93536000: 空のモデルリストは登録拒否されるため、少なくとも1つのモデルを返す
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock_server)
        .await;

    let runtime_port = mock_server.address().port().saturating_sub(1);
    let payload = node_payload(runtime_port);

    // No API key -> 401
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/internal/test/register-node")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Wrong scope -> 403
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/internal/test/register-node")
                .header("authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Correct scope -> 201
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/internal/test/register-node")
                .header("authorization", format!("Bearer {}", node_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn v1_inference_requires_api_inference_scope() {
    let (app, db_pool) = build_app().await;

    let node_key =
        create_api_key(&db_pool, vec![ApiKeyScope::Node]).await;
    let api_key =
        create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;

    let payload = json!({
        "model": "test-model",
        "messages": [
            {"role": "user", "content": "Hello"}
        ]
    });

    // Wrong scope -> 403
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", node_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Correct scope -> authenticated (503 if no nodes)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        response.status() == StatusCode::SERVICE_UNAVAILABLE
            || response.status() == StatusCode::OK,
        "expected authenticated response, got {}",
        response.status()
    );
}

#[tokio::test]
async fn admin_scope_allows_dashboard_overview() {
    let (app, db_pool) = build_app().await;
    let admin_key = create_api_key(&db_pool, vec![ApiKeyScope::Admin]).await;

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

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn v0_health_requires_node_register_scope() {
    let (app, db_pool) = build_app().await;

    let node_key = create_api_key(&db_pool, vec![ApiKeyScope::Node]).await;
    let api_key = create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;

    let payload = node_payload(32769);
    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/internal/test/register-node")
                .header("authorization", format!("Bearer {}", node_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_response.status(), StatusCode::CREATED);

    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_data: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = register_data["node_id"].as_str().unwrap();
    let node_token = register_data["node_token"].as_str().unwrap();

    let health_payload = json!({
        "node_id": node_id,
        "cpu_usage": 0.0,
        "memory_usage": 0.0,
        "active_requests": 0,
        "loaded_models": [],
        "loaded_embedding_models": [],
        "initializing": false
    });

    // No API key -> 401
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Wrong scope -> 403
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Correct scope -> 200
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/health")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", node_key))
                .header("x-node-token", node_token)
                .body(Body::from(serde_json::to_vec(&health_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
