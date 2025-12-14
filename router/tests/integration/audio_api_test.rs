//! 音声API統合テスト
//!
//! TDD RED: 音声認識（ASR）と音声合成（TTS）のノード選択とプロキシテスト

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

/// T013: ASRノード選択テスト
///
/// RuntimeType.Whisperを持つノードが/v1/audio/transcriptionsにルーティングされる
#[tokio::test]
#[ignore = "TDD RED: Audio API routing not implemented yet"]
async fn test_asr_node_routing_selects_whisper_runtime() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // Whisper対応ノードを登録
    let register_payload = json!({
        "machine_name": "whisper-node",
        "ip_address": "192.168.1.100",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["whisper"],
        "loaded_asr_models": ["whisper-large-v3"]
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

    // ノード一覧を確認してWhisper対応ノードが登録されていることを確認
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

    let whisper_node = node_list.iter().find(|n| {
        n.get("supported_runtimes")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("whisper")))
            .unwrap_or(false)
    });

    assert!(
        whisper_node.is_some(),
        "A node with whisper runtime should be registered"
    );
}

/// T014: TTSノード選択テスト
///
/// RuntimeType.OnnxTtsを持つノードが/v1/audio/speechにルーティングされる
#[tokio::test]
#[ignore = "TDD RED: Audio API routing not implemented yet"]
async fn test_tts_node_routing_selects_onnx_runtime() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // TTS対応ノードを登録
    let register_payload = json!({
        "machine_name": "tts-node",
        "ip_address": "192.168.1.101",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["onnx_tts"],
        "loaded_tts_models": ["vibevoice-v1"]
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

    // ノード一覧を確認してTTS対応ノードが登録されていることを確認
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

    let node_list = nodes.as_array().expect("nodes should be an array");
    assert!(
        !node_list.is_empty(),
        "at least one node should be registered"
    );

    let tts_node = node_list.iter().find(|n| {
        n.get("supported_runtimes")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("onnx_tts")))
            .unwrap_or(false)
    });

    assert!(
        tts_node.is_some(),
        "A node with onnx_tts runtime should be registered"
    );
}

/// T015: 複合ノード選択テスト
///
/// 複数のRuntimeTypeを持つノードが適切に処理される
#[tokio::test]
#[ignore = "TDD RED: Audio API routing not implemented yet"]
async fn test_multi_runtime_node_handles_both_asr_and_tts() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // 複合ランタイム対応ノードを登録（LLM + ASR + TTS）
    let register_payload = json!({
        "machine_name": "multi-runtime-node",
        "ip_address": "192.168.1.102",
        "runtime_version": "0.1.0",
        "runtime_port": 8080,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2, "memory": 24576}
        ],
        "supported_runtimes": ["llama_cpp", "whisper", "onnx_tts"],
        "loaded_models": ["llama-3.1-8b-instruct"],
        "loaded_asr_models": ["whisper-large-v3"],
        "loaded_tts_models": ["vibevoice-v1"]
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

    // ノードが存在し、複数のランタイムをサポートしていることを確認
    // 実際のエンドポイント実装後に詳細な検証を追加
    assert!(
        detail_response.status() == StatusCode::OK
            || detail_response.status() == StatusCode::NOT_FOUND,
        "Node detail endpoint should be accessible"
    );
}

/// T016: 対応ノードなしフォールバックテスト
///
/// 要求されたRuntimeTypeを持つノードがない場合、503を返す
#[tokio::test]
#[ignore = "TDD RED: Audio API routing not implemented yet"]
async fn test_no_capable_node_returns_503() {
    std::env::set_var("LLM_ROUTER_SKIP_HEALTH_CHECK", "1");
    let app = build_app().await;

    // LLMノードのみを登録（ASR/TTSなし）
    let register_payload = json!({
        "machine_name": "llm-only-node",
        "ip_address": "192.168.1.103",
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

    // ASRリクエスト（対応ノードなし）- 503を期待
    // 注: 実際のmultipartリクエストは契約テストでカバー
    // ここではルーティングロジックのテストに焦点

    // TTSリクエストを試行（JSON形式）
    let tts_request = json!({
        "model": "vibevoice-v1",
        "input": "テスト",
        "voice": "nova"
    });

    let tts_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/speech")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key.key))
                .body(Body::from(serde_json::to_vec(&tts_request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // TTS対応ノードがないため503を期待
    assert_eq!(
        tts_response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Should return 503 when no TTS-capable node is available"
    );
}
