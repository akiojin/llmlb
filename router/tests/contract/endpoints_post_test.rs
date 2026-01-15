//! Contract Test: POST /v0/endpoints
//!
//! SPEC-66555000: エンドポイント登録API契約テスト

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
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();
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

/// POST /v0/endpoints - 正常系: エンドポイント登録成功
#[tokio::test]
#[serial]
async fn test_create_endpoint_success() {
    let TestApp { app, admin_key } = build_app().await;

    let payload = json!({
        "name": "Test Ollama",
        "base_url": "http://localhost:11434"
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // 契約に基づくレスポンス検証
    assert!(body["id"].is_string(), "id should be a UUID string");
    assert_eq!(body["name"], "Test Ollama");
    assert_eq!(body["base_url"], "http://localhost:11434");
    assert_eq!(body["status"], "pending");
    assert_eq!(body["health_check_interval_secs"], 30);
    assert!(body["last_seen"].is_null());
    assert!(body["last_error"].is_null());
    assert_eq!(body["error_count"], 0);
    assert!(body["registered_at"].is_string());
}

/// POST /v0/endpoints - 正常系: オプションフィールド付き登録
#[tokio::test]
#[serial]
async fn test_create_endpoint_with_optional_fields() {
    let TestApp { app, admin_key } = build_app().await;

    let payload = json!({
        "name": "Production vLLM",
        "base_url": "http://192.168.1.100:8000",
        "api_key": "sk-secret-key",
        "health_check_interval_secs": 60,
        "notes": "Production server"
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(body["name"], "Production vLLM");
    assert_eq!(body["health_check_interval_secs"], 60);
    assert_eq!(body["notes"], "Production server");
    // api_keyはシリアライズされない（セキュリティ）
    assert!(body.get("api_key").is_none() || body["api_key"].is_null());
}

/// POST /v0/endpoints - 異常系: 名前が空
#[tokio::test]
#[serial]
async fn test_create_endpoint_empty_name() {
    let TestApp { app, admin_key } = build_app().await;

    let payload = json!({
        "name": "",
        "base_url": "http://localhost:11434"
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// POST /v0/endpoints - 異常系: 不正なURL
#[tokio::test]
#[serial]
async fn test_create_endpoint_invalid_url() {
    let TestApp { app, admin_key } = build_app().await;

    let payload = json!({
        "name": "Invalid",
        "base_url": "not-a-valid-url"
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// POST /v0/endpoints - 異常系: URL重複
#[tokio::test]
#[serial]
#[ignore = "TDD RED: URL重複チェック未実装"]
async fn test_create_endpoint_duplicate_url() {
    let TestApp { app, admin_key } = build_app().await;

    let payload = json!({
        "name": "First",
        "base_url": "http://localhost:11434"
    });

    // 最初の登録
    let response = app
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
    assert_eq!(response.status(), StatusCode::CREATED);

    // 重複登録
    let dup_payload = json!({
        "name": "Second",
        "base_url": "http://localhost:11434"
    });

    let dup_response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&dup_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(dup_response.status(), StatusCode::CONFLICT);

    let body = to_bytes(dup_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["error"]["code"].is_string());
}

/// POST /v0/endpoints - 異常系: 認証なし
#[tokio::test]
#[serial]
async fn test_create_endpoint_unauthorized() {
    let TestApp { app, .. } = build_app().await;

    let payload = json!({
        "name": "Test",
        "base_url": "http://localhost:11434"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/endpoints")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// POST /v0/endpoints - 異常系: ヘルスチェック間隔の範囲外
#[tokio::test]
#[serial]
async fn test_create_endpoint_invalid_health_check_interval() {
    let TestApp { app, admin_key } = build_app().await;

    // 範囲下限（10未満）
    let payload = json!({
        "name": "Test",
        "base_url": "http://localhost:11434",
        "health_check_interval_secs": 5
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 範囲上限（300超）
    let payload = json!({
        "name": "Test2",
        "base_url": "http://localhost:11435",
        "health_check_interval_secs": 500
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
