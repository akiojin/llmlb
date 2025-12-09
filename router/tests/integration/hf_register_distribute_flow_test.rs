use axum::{body::to_bytes, http::Request, Router};
use llm_router::{
    api,
    api::models::clear_hf_cache,
    balancer::LoadManager,
    registry::NodeRegistry,
    AppState,
};
use serial_test::serial;
use serde_json::json;
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

/// HFカタログ→登録→全ノードダウンロード→/v1/models までの一連フロー
#[tokio::test]
#[serial]
async fn hf_catalog_register_distribute_flow() {
    clear_hf_cache();
    let mock = MockServer::start().await;

    // カタログレスポンス
    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![json!({
            "modelId": "test/repo",
            "tags": ["gguf"],
            "siblings": [ {"rfilename": "model.gguf", "size": 1024u64 * 1024 * 1024 } ],
            "lastModified": "2024-02-01T00:00:00Z"
        })]))
        .mount(&mock)
        .await;

    // HEAD for register
    Mock::given(method("HEAD"))
        .and(path("/test/repo/resolve/main/model.gguf"))
        .respond_with(ResponseTemplate::new(200).insert_header("Content-Length", (1024u64 * 1024 * 1024).to_string()))
        .mount(&mock)
        .await;

    std::env::set_var("HF_BASE_URL", mock.uri());
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    std::env::set_var("LLM_ROUTER_SKIP_API_KEY", "1");

    let app = build_app().await;

    // ノード登録（GPUメモリ8GB）
    let register_payload = json!({
        "machine_name": "node-1",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": 11434,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1, "memory": 8 * 1024 * 1024 * 1024u64}
        ]
    });
    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register_response.status(), axum::http::StatusCode::CREATED);
    let body = to_bytes(register_response.into_body(), usize::MAX).await.unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"].as_str().unwrap().to_string();

    // カタログ取得（cached=false）
    let available = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/models/available?source=hf")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(available.status(), axum::http::StatusCode::OK);
    let available_body = to_bytes(available.into_body(), usize::MAX).await.unwrap();
    let available_json: serde_json::Value = serde_json::from_slice(&available_body).unwrap();
    assert_eq!(available_json["cached"], serde_json::Value::Bool(false));

    // 登録
    let register_model = json!({
        "repo": "test/repo",
        "filename": "model.gguf"
    });
    let reg = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&register_model).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg.status(), axum::http::StatusCode::CREATED);

    // 全ノードにダウンロード
    let distribute = json!({
        "model_name": "hf/test/repo/model.gguf",
        "target": "all",
        "node_ids": []
    });
    let distribute_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/models/distribute")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(serde_json::to_vec(&distribute).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(distribute_res.status(), axum::http::StatusCode::ACCEPTED);
    let dist_body = to_bytes(distribute_res.into_body(), usize::MAX).await.unwrap();
    let dist_json: serde_json::Value = serde_json::from_slice(&dist_body).unwrap();
    let task_ids = dist_json["task_ids"].as_array().unwrap();
    assert_eq!(task_ids.len(), 1);
    let task_id = task_ids[0].as_str().unwrap();

    // タスク詳細
    let task_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/tasks/{}", task_id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(task_res.status(), axum::http::StatusCode::OK);
    let task_body = to_bytes(task_res.into_body(), usize::MAX).await.unwrap();
    let task_json: serde_json::Value = serde_json::from_slice(&task_body).unwrap();
    assert_eq!(task_json["model_name"], "hf/test/repo/model.gguf");
    assert_eq!(task_json["node_id"], node_id);

    // /v1/models に登録モデルが含まれる
    let models_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_res.status(), axum::http::StatusCode::OK);
    let models_body = to_bytes(models_res.into_body(), usize::MAX).await.unwrap();
    let models_json: serde_json::Value = serde_json::from_slice(&models_body).unwrap();
    assert!(
        models_json["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "hf/test/repo/model.gguf" && m["download_url"].is_string()),
        "/v1/models must include registered HF model"
    );

    std::env::remove_var("HF_BASE_URL");
    std::env::remove_var("LLM_ROUTER_SKIP_HEALTH_CHECK");
    std::env::remove_var("LLM_ROUTER_SKIP_API_KEY");
    clear_hf_cache();
}
