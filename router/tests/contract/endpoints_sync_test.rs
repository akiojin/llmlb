//! Contract Test: POST /v0/endpoints/:id/sync
//!
//! SPEC-66555000: エンドポイントモデル同期API契約テスト

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
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

/// POST /v0/endpoints/:id/sync - 正常系: モデル同期成功
#[tokio::test]
#[serial]
async fn test_endpoint_sync_success() {
    let mock = MockServer::start().await;

    // モックエンドポイントのレスポンス設定
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model"},
                {"id": "gpt-3.5-turbo", "object": "model"},
                {"id": "text-embedding-ada-002", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Mock Endpoint",
        "base_url": mock.uri()
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

    // モデル同期
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri(format!("/v0/endpoints/{}/sync", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // 契約に基づくレスポンス検証
    assert!(body["synced_models"].is_array());
    assert!(body["added"].is_number());
    assert!(body["removed"].is_number());
    assert!(body["updated"].is_number());

    // 同期されたモデルの検証
    let synced_models = body["synced_models"].as_array().unwrap();
    assert!(!synced_models.is_empty());

    for model in synced_models {
        assert!(model["model_id"].is_string());
        // capabilitiesは配列またはnull
        assert!(model["capabilities"].is_array() || model["capabilities"].is_null());
    }
}

/// POST /v0/endpoints/:id/sync - 異常系: 存在しないエンドポイント
#[tokio::test]
#[serial]
async fn test_endpoint_sync_not_found() {
    let TestApp { app, admin_key } = build_app().await;

    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri(format!("/v0/endpoints/{}/sync", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// POST /v0/endpoints/:id/sync - 異常系: エンドポイントがオフライン
#[tokio::test]
#[serial]
async fn test_endpoint_sync_offline() {
    let TestApp { app, admin_key } = build_app().await;

    // 到達不能なエンドポイントを登録
    let payload = json!({
        "name": "Offline Endpoint",
        "base_url": "http://127.0.0.1:59999"
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

    // モデル同期（失敗が予想される）
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri(format!("/v0/endpoints/{}/sync", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 503 Service Unavailableが期待される
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// POST /v0/endpoints/:id/sync - 異常系: 認証なし
#[tokio::test]
#[serial]
async fn test_endpoint_sync_unauthorized() {
    let TestApp { app, .. } = build_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v0/endpoints/{}/sync", Uuid::new_v4()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// POST /v0/endpoints/:id/sync - 正常系: 空のモデル一覧
#[tokio::test]
#[serial]
async fn test_endpoint_sync_empty_models() {
    let mock = MockServer::start().await;

    // 空のモデル一覧を返すモック
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    let TestApp { app, admin_key } = build_app().await;

    // エンドポイント登録
    let payload = json!({
        "name": "Empty Models Endpoint",
        "base_url": mock.uri()
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

    // モデル同期
    let response = app
        .oneshot(
            admin_request(&admin_key)
                .method("POST")
                .uri(format!("/v0/endpoints/{}/sync", endpoint_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    assert!(body["synced_models"].is_array());
    assert_eq!(body["synced_models"].as_array().unwrap().len(), 0);
}
