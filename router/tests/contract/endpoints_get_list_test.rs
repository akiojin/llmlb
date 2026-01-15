//! Contract Test: GET /v0/endpoints
//!
//! SPEC-66555000: エンドポイント一覧取得API契約テスト

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

/// GET /v0/endpoints - 正常系: 空の一覧
#[tokio::test]
#[serial]
async fn test_list_endpoints_empty() {
    let TestApp { app, admin_key } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/endpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert!(body["endpoints"].is_array());
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 0);
    assert_eq!(body["total"], 0);
}

/// GET /v0/endpoints - 正常系: 複数エンドポイントの一覧
#[tokio::test]
#[serial]
async fn test_list_endpoints_multiple() {
    let TestApp { app, admin_key } = build_app().await;

    // 2つのエンドポイントを登録
    for i in 1..=2 {
        let payload = json!({
            "name": format!("Endpoint {}", i),
            "base_url": format!("http://localhost:{}", 11434 + i)
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
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // 一覧取得
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/endpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    let endpoints = body["endpoints"].as_array().unwrap();
    assert_eq!(endpoints.len(), 2);
    assert_eq!(body["total"], 2);

    // 各エンドポイントの構造検証
    for endpoint in endpoints {
        assert!(endpoint["id"].is_string());
        assert!(endpoint["name"].is_string());
        assert!(endpoint["base_url"].is_string());
        assert!(endpoint["status"].is_string());
        assert!(endpoint["health_check_interval_secs"].is_number());
        assert!(endpoint["registered_at"].is_string());
        // model_countが含まれる
        assert!(endpoint["model_count"].is_number());
    }
}

/// GET /v0/endpoints - 正常系: ステータスフィルタ
#[tokio::test]
#[serial]
async fn test_list_endpoints_filter_by_status() {
    let TestApp { app, admin_key } = build_app().await;

    // エンドポイントを登録（初期状態はpending）
    let payload = json!({
        "name": "Test Endpoint",
        "base_url": "http://localhost:11434"
    });

    let _ = app
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

    // pendingでフィルタ
    let response = app
        .clone()
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/endpoints?status=pending")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 1);

    // onlineでフィルタ（該当なし）
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("GET")
                .uri("/v0/endpoints?status=online")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 0);
}

/// GET /v0/endpoints - 異常系: 認証なし
#[tokio::test]
#[serial]
async fn test_list_endpoints_unauthorized() {
    let TestApp { app, .. } = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/endpoints")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
