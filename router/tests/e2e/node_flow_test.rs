//! エンドポイント管理E2Eテスト
//!
//! SPEC-66555000: NodeRegistryは廃止されました。
//! GET系の管理APIは引き続き存在しますが、内部実装はEndpointRegistryベースに変更されています。
//!
//! NOTE: POST /v0/nodes と POST /v0/health は完全に廃止されました。
//! エンドポイント登録は /v0/endpoints API で行います。

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use crate::support;

async fn build_app() -> (Router, sqlx::SqlitePool, String) {
    let db_pool = support::router::create_test_db_pool().await;
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = support::router::test_jwt_secret();

    let state = AppState {
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

/// SPEC-66555000: GET /v0/nodes は廃止され、/v0/endpoints に移行
/// このテストはEndpointRegistry APIへの移行を確認するためのプレースホルダーです
#[tokio::test]
#[ignore = "SPEC-66555000: /v0/nodes is deprecated, use /v0/endpoints instead"]
async fn test_list_nodes() {
    let (_app, _db_pool, _admin_key) = build_app().await;
    // TODO: EndpointRegistry APIのテストを実装
}

#[tokio::test]
#[ignore = "SPEC-66555000: /v0/nodes/metrics is deprecated"]
async fn test_list_node_metrics() {
    let (_app, _db_pool, _admin_key) = build_app().await;
    // TODO: EndpointRegistryベースのメトリクステストを実装
}

#[tokio::test]
#[ignore = "SPEC-66555000: /v0/metrics/summary endpoint is deprecated"]
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
