use axum::{body::to_bytes, http::Request, Router};
use llm_router::{
    api,
    api::models::clear_hf_cache,
    balancer::LoadManager,
    registry::NodeRegistry,
    AppState,
};
use serial_test::serial;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_app() -> Router {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let task_manager = llm_router::tasks::DownloadTaskManager::new();
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        task_manager,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

#[tokio::test]
#[serial]
async fn available_models_falls_back_to_cache_on_429() {
    clear_hf_cache();
    let mock = MockServer::start().await;

    // 1回目: 正常レスポンス（キャッシュを作る）
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![serde_json::json!({
            "modelId": "Test/Model-1",
            "tags": ["gguf"],
            "siblings": [ {"rfilename": "model.q4.gguf", "size": 1234u64} ],
            "lastModified": "2024-01-01T00:00:00Z"
        })]))
        .expect(1)
        .mount(&mock)
        .await;

    // 2回目: 429 を返してキャッシュフォールバックを期待
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&mock)
        .await;

    std::env::set_var("HF_BASE_URL", mock.uri());
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");

    let app = build_app().await;

    // 1回目: キャッシュ作成（cached=false）
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/models/available?source=hf")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let body = to_bytes(first.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["source"], "hf");
    assert_eq!(json["cached"], serde_json::Value::Bool(false));

    // 2回目: 429だがキャッシュで成功し、cached=true になる
    let second = app
        .oneshot(
            Request::builder()
                .uri("/api/models/available?source=hf")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second.status(), axum::http::StatusCode::OK);
    let body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["cached"], serde_json::Value::Bool(true));
    assert!(json["models"].as_array().map(|a| !a.is_empty()).unwrap_or(false));

    // 後処理: 環境とキャッシュをリセット
    std::env::remove_var("HF_BASE_URL");
    clear_hf_cache();
}
