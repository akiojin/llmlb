//! 画像API統合テスト
//!
//! TDD RED: 画像生成（StableDiffusion）のノード選択とプロキシテスト

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serde_json::json;
use tower::ServiceExt;

use crate::support::{
    admin::{admin_request, approve_node},
    http,
};

async fn build_app() -> Router {
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
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
    };

    api::create_router(state)
}

async fn spawn_image_stub() -> http::TestServerGuard {
    let router = Router::new()
        .route("/v1/images/generations", post(image_gen_handler))
        .route("/v1/models", get(models_handler));
    http::spawn_router_guarded(router).await
}

fn runtime_port_for_stub(stub: &http::TestServerGuard) -> u16 {
    // Router derives the node API port as runtime_port + 1.
    stub.addr().port() - 1
}

fn node_register_request() -> axum::http::request::Builder {
    Request::builder().header("x-api-key", "sk_debug_node")
}

async fn image_gen_handler(Json(_payload): Json<serde_json::Value>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "created": 0,
            "data": [{"url": "http://example.com/generated.png"}]
        })),
    )
        .into_response()
}

async fn models_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "data": [] }))).into_response()
}

/// IMG001: 画像生成ノード選択テスト
///
/// RuntimeType::StableDiffusionを持つノードが/v1/images/generationsにルーティングされる
#[tokio::test]
async fn test_image_gen_node_routing_selects_stable_diffusion_runtime() {
    let app = build_app().await;
    let stub = spawn_image_stub().await;

    // StableDiffusion対応ノードを登録
    let register_payload = json!({
        "machine_name": "sd-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1, "memory": 24576}
        ],
        "supported_runtimes": ["stable_diffusion"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"].as_str().expect("node_id should exist");
    approve_node(&app, node_id).await;

    let image_request = json!({
        "model": "stable-diffusion-xl",
        "prompt": "A white cat",
        "n": 1,
        "size": "1024x1024"
    });

    let image_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
                .body(Body::from(serde_json::to_vec(&image_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(image_response.status(), StatusCode::OK);
    let body = to_bytes(image_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(payload.get("data").is_some());
}

/// IMG002: 複合ランタイムノードテスト
///
/// LLM + StableDiffusionを持つノードが適切に処理される
#[tokio::test]
async fn test_multi_runtime_node_handles_llm_and_image() {
    let app = build_app().await;
    let stub = spawn_image_stub().await;

    // 複合ランタイム対応ノードを登録（LLM + StableDiffusion）
    let register_payload = json!({
        "machine_name": "multi-runtime-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2, "memory": 24576}
        ],
        "supported_runtimes": ["llama_cpp", "stable_diffusion"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"].as_str().expect("node_id should exist");
    approve_node(&app, node_id).await;

    let nodes_response = app
        .clone()
        .oneshot(
            admin_request()
                .method("GET")
                .uri("/v0/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(nodes_response.status(), StatusCode::OK);
    let body = to_bytes(nodes_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let nodes: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_list = nodes.as_array().expect("nodes should be an array");
    let node = node_list
        .iter()
        .find(|n| n.get("machine_name").and_then(|v| v.as_str()) == Some("multi-runtime-node"))
        .expect("multi-runtime node should exist");
    let runtimes = node
        .get("supported_runtimes")
        .and_then(|r| r.as_array())
        .expect("supported_runtimes should be array");
    assert!(runtimes.iter().any(|v| v.as_str() == Some("llama_cpp")));
    assert!(runtimes
        .iter()
        .any(|v| v.as_str() == Some("stable_diffusion")));

    let image_request = json!({
        "model": "stable-diffusion-xl",
        "prompt": "A white cat",
        "n": 1,
        "size": "1024x1024"
    });

    let image_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
                .body(Body::from(serde_json::to_vec(&image_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(image_response.status(), StatusCode::OK);
    let body = to_bytes(image_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(payload.get("data").is_some());
}

/// IMG003: 画像生成対応ノードなしテスト
///
/// StableDiffusion対応ノードがない場合、503を返す
#[tokio::test]
async fn test_no_image_capable_node_returns_503() {
    let app = build_app().await;
    let stub = spawn_image_stub().await;

    // LLMノードのみを登録（StableDiffusionなし）
    let register_payload = json!({
        "machine_name": "llm-only-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["llama_cpp"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);
    let body = to_bytes(register_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let node_id = node["node_id"].as_str().expect("node_id should exist");
    approve_node(&app, node_id).await;
    // 画像生成リクエストを試行（JSON形式）
    let image_request = json!({
        "model": "stable-diffusion-xl",
        "prompt": "A white cat",
        "n": 1,
        "size": "1024x1024"
    });

    let image_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
                .body(Body::from(serde_json::to_vec(&image_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // StableDiffusion対応ノードがないため503を期待
    assert_eq!(
        image_response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Should return 503 when no StableDiffusion-capable node is available"
    );
}

/// IMG004: 画像生成APIルート存在テスト
///
/// /v1/images/generations, /v1/images/edits, /v1/images/variationsルートが存在する
#[tokio::test]
async fn test_image_api_routes_exist() {
    let app = build_app().await;
    // /v1/images/generations (POST)
    let gen_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "model": "stable-diffusion-xl",
                        "prompt": "test"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 404でないことを確認（503 Service Unavailableは許容）
    assert_ne!(
        gen_response.status(),
        StatusCode::NOT_FOUND,
        "/v1/images/generations route should exist"
    );

    // /v1/images/edits (POST) - multipartなので空ボディでもルートは存在確認
    let edits_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/edits")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(
        edits_response.status(),
        StatusCode::NOT_FOUND,
        "/v1/images/edits route should exist"
    );

    // /v1/images/variations (POST) - multipartなので空ボディでもルートは存在確認
    let variations_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/variations")
                .header("x-api-key", "sk_debug")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(
        variations_response.status(),
        StatusCode::NOT_FOUND,
        "/v1/images/variations route should exist"
    );
}

/// IMG005: 認証なし画像生成リクエストテスト
///
/// 認証ヘッダーなしで401を返す
#[tokio::test]
async fn test_image_generation_without_auth_returns_401() {
    let app = build_app().await;

    let image_request = json!({
        "model": "stable-diffusion-xl",
        "prompt": "A white cat"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                // No Authorization header
                .body(Body::from(serde_json::to_vec(&image_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Should return 401 when no auth header is provided"
    );
}
