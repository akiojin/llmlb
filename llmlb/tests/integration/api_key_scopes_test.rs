//! Integration Test: APIキーのスコープ制御
//!
//! /api と /v1 の各エンドポイントでスコープが正しく強制されることを確認する。
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
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool) {
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

    (api::create_app(state), db_pool)
}

async fn create_admin_user(db_pool: &sqlx::SqlitePool) -> uuid::Uuid {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created = llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin).await;
    if let Ok(user) = created {
        return user.id;
    }
    llmlb::db::users::find_by_username(db_pool, "admin")
        .await
        .unwrap()
        .unwrap()
        .id
}

async fn create_api_key(db_pool: &sqlx::SqlitePool, scopes: Vec<ApiKeyScope>) -> String {
    let admin_id = create_admin_user(db_pool).await;
    let api_key = llmlb::db::api_keys::create(db_pool, "test-key", admin_id, None, scopes)
        .await
        .expect("create api key");
    api_key.key
}

#[tokio::test]
async fn endpoints_create_requires_admin_scope() {
    let (app, db_pool) = build_app().await;

    let node_key = create_api_key(&db_pool, vec![ApiKeyScope::Endpoint]).await;
    let api_key = create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;
    let admin_key = create_api_key(&db_pool, vec![ApiKeyScope::Admin]).await;

    let mock_server = MockServer::start().await;
    // 検出経路のタイムアウトを避けるため最小限のレスポンスを用意
    Mock::given(method("GET"))
        .and(path("/api/system"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;
    // SPEC-93536000: 空のモデルリストは登録拒否されるため、少なくとも1つのモデルを返す
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock_server)
        .await;

    let payload = json!({
        "name": "scope-endpoint",
        "base_url": format!("http://{}", mock_server.address()),
        "health_check_interval_secs": 30
    });

    // No API key -> 401
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/endpoints")
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
                .uri("/api/endpoints")
                .header("authorization", format!("Bearer {}", api_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Endpoint scope -> 403 (admin required)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/endpoints")
                .header("authorization", format!("Bearer {}", node_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Admin scope -> 201
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/endpoints")
                .header("authorization", format!("Bearer {}", admin_key))
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

    let node_key = create_api_key(&db_pool, vec![ApiKeyScope::Endpoint]).await;
    let api_key = create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;

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
        matches!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE | StatusCode::OK | StatusCode::NOT_FOUND
        ),
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
                .uri("/api/dashboard/overview")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn v0_models_requires_endpoint_scope() {
    let (app, db_pool) = build_app().await;

    let node_key = create_api_key(&db_pool, vec![ApiKeyScope::Endpoint]).await;
    let api_key = create_api_key(&db_pool, vec![ApiKeyScope::Api]).await;

    // No API key -> 401
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models")
                .body(Body::empty())
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
                .method("GET")
                .uri("/api/models")
                .header("authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Correct scope -> 200
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models")
                .header("authorization", format!("Bearer {}", node_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
