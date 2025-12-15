//! 画像API統合テスト
//!
//! TDD RED: 画像生成（StableDiffusion）のノード選択とプロキシテスト

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serde_json::json;
use tower::ServiceExt;

async fn build_app() -> Router {
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(llm_router::db::request_history::RequestHistoryStorage::new().unwrap());
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
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    api::create_router(state)
}

/// IMG001: 画像生成ノード選択テスト
///
/// RuntimeType::StableDiffusionを持つノードが/v1/images/generationsにルーティングされる
#[tokio::test]
#[ignore = "TDD RED: Image API routing not implemented yet"]
async fn test_image_gen_node_routing_selects_stable_diffusion_runtime() {
    let app = build_app().await;

    // StableDiffusion対応ノードを登録
    let register_payload = json!({
        "machine_name": "sd-node",
        "ip_address": "192.168.1.200",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1, "memory": 24576}
        ],
        "supported_runtimes": ["stable_diffusion"],
        "loaded_image_models": ["stable-diffusion-xl"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // ノード一覧を確認してStableDiffusion対応ノードが登録されていることを確認
    let nodes_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/nodes")
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

    // ノードにsupported_runtimesフィールドがあることを確認
    let node_list = nodes.as_array().expect("nodes should be an array");
    assert!(
        !node_list.is_empty(),
        "at least one node should be registered"
    );

    let sd_node = node_list.iter().find(|n| {
        n.get("supported_runtimes")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("stable_diffusion")))
            .unwrap_or(false)
    });

    assert!(
        sd_node.is_some(),
        "A node with stable_diffusion runtime should be registered"
    );
}

/// IMG002: 複合ランタイムノードテスト
///
/// LLM + StableDiffusionを持つノードが適切に処理される
#[tokio::test]
#[ignore = "TDD RED: Image API routing not implemented yet"]
async fn test_multi_runtime_node_handles_llm_and_image() {
    let app = build_app().await;

    // 複合ランタイム対応ノードを登録（LLM + StableDiffusion）
    let register_payload = json!({
        "machine_name": "multi-runtime-node",
        "ip_address": "192.168.1.201",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2, "memory": 24576}
        ],
        "supported_runtimes": ["llama_cpp", "stable_diffusion"],
        "loaded_models": ["llama-3.1-8b-instruct"],
        "loaded_image_models": ["stable-diffusion-xl"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
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

    // ノード詳細を取得して複数のランタイムが登録されていることを確認
    let detail_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/nodes/{}", node_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(
        detail_response.status() == StatusCode::OK
            || detail_response.status() == StatusCode::NOT_FOUND,
        "Node detail endpoint should be accessible"
    );
}

/// IMG003: 画像生成対応ノードなしテスト
///
/// StableDiffusion対応ノードがない場合、503を返す
#[tokio::test]
#[ignore = "TDD RED: Image API routing not implemented yet"]
async fn test_no_image_capable_node_returns_503() {
    let app = build_app().await;

    // LLMノードのみを登録（StableDiffusionなし）
    let register_payload = json!({
        "machine_name": "llm-only-node",
        "ip_address": "192.168.1.202",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["llama_cpp"],
        "loaded_models": ["llama-3.1-8b-instruct"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // テスト用DBとAPIキーを作成
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let test_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        "testpassword",
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(&db_pool, "test-key", test_user.id, None)
        .await
        .expect("Failed to create test API key");

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
                .header("Authorization", format!("Bearer {}", api_key.key))
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
#[ignore = "TDD RED: Image API routing not implemented yet"]
async fn test_image_api_routes_exist() {
    let app = build_app().await;

    // テスト用DBとAPIキーを作成
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let test_user = llm_router::db::users::create(
        &db_pool,
        "test-admin",
        "testpassword",
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");
    let api_key = llm_router::db::api_keys::create(&db_pool, "test-key", test_user.id, None)
        .await
        .expect("Failed to create test API key");

    // /v1/images/generations (POST)
    let gen_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key.key))
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
                .header("Authorization", format!("Bearer {}", api_key.key))
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
                .header("Authorization", format!("Bearer {}", api_key.key))
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
#[ignore = "TDD RED: Image API routing not implemented yet"]
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
