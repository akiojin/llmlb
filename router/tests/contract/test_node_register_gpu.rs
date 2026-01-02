//! Contract Test: GPU必須ノード登録
//!
//! GPU情報を含むノードのみが登録され、レスポンスへGPU情報が反映されることを検証する。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::{ApiKeyScope, UserRole};
use serde_json::json;
use serial_test::serial;
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn build_app() -> (Router, String) {
    // テスト用に一時ディレクトリを設定
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
        db_pool,
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

    (api::create_router(state), admin_key)
}

#[tokio::test]
#[serial]
async fn register_gpu_node_success() {
    // モックサーバーを起動してヘルスチェックに応答
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock_server)
        .await;

    // モックサーバーのポートを取得（ルーターは runtime_port + 1 をAPIポートとして使用）
    let mock_port = mock_server.address().port();
    let runtime_port = mock_port - 1;

    let (app, admin_key) = build_app().await;

    let payload = json!({
        "machine_name": "gpu-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.42",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2}
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let body = to_bytes(list_response.into_body(), 1024).await.unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(nodes.is_array(), "expected array response");
    let first = nodes
        .as_array()
        .and_then(|list| list.first())
        .cloned()
        .expect("node must exist");
    assert_eq!(first["machine_name"], "gpu-node");
    assert_eq!(first["gpu_available"], true);
    assert!(
        first["gpu_devices"].is_array(),
        "gpu_devices should be present"
    );
    let gpu_devices = first["gpu_devices"].as_array().unwrap();
    assert_eq!(gpu_devices.len(), 1);
    assert_eq!(gpu_devices[0]["model"], "NVIDIA RTX 4090");
    assert_eq!(gpu_devices[0]["count"], 2);
}

#[tokio::test]
#[serial]
async fn register_gpu_node_missing_devices_is_rejected() {
    // GPUデバイスが空の場合、ヘルスチェック前にバリデーションで拒否される
    let (app, admin_key) = build_app().await;

    let payload = json!({
        "machine_name": "cpu-only",
        "ip_address": "10.0.0.20",
        "runtime_version": "0.1.42",
        "runtime_port": 32768,
        "gpu_available": true,
        "gpu_devices": []
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v0/nodes")
                .header("authorization", format!("Bearer {}", admin_key))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    // Security: external_message() returns generic error to prevent information disclosure
    assert_eq!(error["error"], "Request error");
}
