#![allow(deprecated)] // NodeRegistry → EndpointRegistry migration in progress

//! Contract Test: PUT /v0/endpoints/:id
//!
//! SPEC-66555000: エンドポイント更新API契約テスト

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::{json, Value};
use serial_test::serial;
use tower::ServiceExt;
use uuid::Uuid;

struct TestApp {
    app: Router,
    admin_key: String,
}

async fn build_app() -> TestApp {
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);

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
        db_pool: db_pool.clone(),
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

    let app = api::create_router(state);
    TestApp { app, admin_key }
}

fn admin_request(admin_key: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", admin_key))
}

/// PUT /v0/endpoints/:id - 正常系: 名前の更新
#[tokio::test]
#[serial]
async fn test_update_endpoint_name() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Original Name",
        "base_url": "http://localhost:11434"
    });

    let create_response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body: Value = serde_json::from_slice(&create_body).unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // 名前を更新
    let update_payload = json!({
        "name": "Updated Name"
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", endpoint_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(body["id"], endpoint_id);
    assert_eq!(body["name"], "Updated Name");
    assert_eq!(body["base_url"], "http://localhost:11434");
}

/// PUT /v0/endpoints/:id - 正常系: ヘルスチェック間隔の更新
#[tokio::test]
#[serial]
async fn test_update_endpoint_health_check_interval() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Test",
        "base_url": "http://localhost:11434"
    });

    let create_response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body: Value = serde_json::from_slice(&create_body).unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // ヘルスチェック間隔を更新
    let update_payload = json!({
        "health_check_interval_secs": 120
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", endpoint_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(body["health_check_interval_secs"], 120);
}

/// PUT /v0/endpoints/:id - 正常系: notesの更新（nullで削除）
#[tokio::test]
#[serial]
async fn test_update_endpoint_notes() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録（notes付き）
    let payload = json!({
        "name": "Test",
        "base_url": "http://localhost:11434",
        "notes": "Initial notes"
    });

    let create_response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body: Value = serde_json::from_slice(&create_body).unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // notesをnullで削除
    let update_payload = json!({
        "notes": null
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", endpoint_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert!(body["notes"].is_null());
}

/// PUT /v0/endpoints/:id - 異常系: 存在しないID
#[tokio::test]
#[serial]
async fn test_update_endpoint_not_found() {
    let TestApp { app, admin_key } = build_app().await;

    let update_payload = json!({
        "name": "Updated"
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", Uuid::new_v4()))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// PUT /v0/endpoints/:id - 異常系: バリデーションエラー（空の名前）
#[tokio::test]
#[serial]
async fn test_update_endpoint_validation_error() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Test",
        "base_url": "http://localhost:11434"
    });

    let create_response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body: Value = serde_json::from_slice(&create_body).unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // 空の名前で更新
    let update_payload = json!({
        "name": ""
    });

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", endpoint_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// PUT /v0/endpoints/:id - 異常系: 認証なし
#[tokio::test]
#[serial]
async fn test_update_endpoint_unauthorized() {
    let TestApp { app, .. } = build_app().await;

    let update_payload = json!({
        "name": "Updated"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v0/endpoints/{}", Uuid::new_v4()))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
