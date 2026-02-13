//! Contract Test: GET /api/endpoints/:id
//!
//! SPEC-66555000: エンドポイント詳細取得API契約テスト
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use llmlb::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use serde_json::{json, Value};
use serial_test::serial;
use std::sync::Arc;
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
    std::env::set_var("LLMLB_DATA_DIR", &temp_dir);

    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
    let http_client = reqwest::Client::new();
    let inference_gate = llmlb::inference_gate::InferenceGate::default();
    let shutdown = llmlb::shutdown::ShutdownController::default();
    let update_manager = llmlb::update::UpdateManager::new(
        http_client.clone(),
        inference_gate.clone(),
        shutdown.clone(),
    )
    .expect("Failed to create update manager");
    let state = AppState {
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client,
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
        inference_gate,
        shutdown,
        update_manager,
    };

    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let admin_user =
        llmlb::db::users::create(&state.db_pool, "admin", &password_hash, UserRole::Admin)
            .await
            .expect("create admin user");
    let admin_key = llmlb::db::api_keys::create(
        &state.db_pool,
        "admin-key",
        admin_user.id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key")
    .key;

    let app = api::create_app(state);
    TestApp { app, admin_key }
}

fn admin_request(admin_key: &str) -> axum::http::request::Builder {
    Request::builder().header("authorization", format!("Bearer {}", admin_key))
}

/// GET /api/endpoints/:id - 正常系: エンドポイント詳細取得
#[tokio::test]
#[serial]
async fn test_get_endpoint_detail_success() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Test Ollama",
        "base_url": "http://localhost:11434",
        "notes": "Test notes"
    });

    let create_response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/api/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_body: Value = serde_json::from_slice(&create_body).unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // 詳細取得
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri(format!("/api/endpoints/{}", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // 契約に基づくレスポンス検証
    assert_eq!(body["id"], endpoint_id);
    assert_eq!(body["name"], "Test Ollama");
    assert_eq!(body["base_url"], "http://localhost:11434");
    assert_eq!(body["status"], "pending");
    assert_eq!(body["health_check_interval_secs"], 30);
    assert!(body["last_seen"].is_null());
    assert!(body["last_error"].is_null());
    assert_eq!(body["error_count"], 0);
    assert!(body["registered_at"].is_string());
    assert_eq!(body["notes"], "Test notes");
    // modelsフィールドが含まれる
    assert!(body["models"].is_array());
}

/// GET /api/endpoints/:id - 異常系: 存在しないID
#[tokio::test]
#[serial]
async fn test_get_endpoint_detail_not_found() {
    let TestApp { app, admin_key } = build_app().await;

    let non_existent_id = Uuid::new_v4();

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri(format!("/api/endpoints/{}", non_existent_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// GET /api/endpoints/:id - 異常系: 不正なUUID形式
#[tokio::test]
#[serial]
async fn test_get_endpoint_detail_invalid_uuid() {
    let TestApp { app, admin_key } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/api/endpoints/not-a-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 不正なUUIDは400または404
    assert!(
        response.status() == StatusCode::BAD_REQUEST || response.status() == StatusCode::NOT_FOUND
    );
}

/// GET /api/endpoints/:id - 異常系: 認証なし
#[tokio::test]
#[serial]
async fn test_get_endpoint_detail_unauthorized() {
    let TestApp { app, .. } = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/endpoints/{}", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
