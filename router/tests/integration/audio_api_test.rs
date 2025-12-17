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
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

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

async fn start_mock_node_models_endpoint() -> (MockServer, u16) {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock_server)
        .await;

    let mock_port = mock_server.address().port();
    let runtime_port = mock_port
        .checked_sub(1)
        .expect("mock_server port must be > 0");

    (mock_server, runtime_port)
}

/// T013: ASRノード選択テスト
///
/// RuntimeType::WhisperCppを持つノードが/v1/audio/transcriptionsにルーティングされる
#[tokio::test]
async fn test_asr_node_routing_selects_whisper_runtime() {
    let (_mock_server, runtime_port) = start_mock_node_models_endpoint().await;
    let app = build_app().await;

    // Whisper対応ノードを登録
    let register_payload = json!({
        "machine_name": "whisper-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["whisper_cpp"],
        "loaded_asr_models": ["whisper-large-v3"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
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
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("whisper_cpp")))
            .unwrap_or(false)
    });

    assert!(
        whisper_node.is_some(),
        "A node with whisper_cpp capability should be registered"
    );
}

/// T014: TTSノード選択テスト
///
/// RuntimeType::OnnxRuntimeを持つノードが/v1/audio/speechにルーティングされる
#[tokio::test]
async fn test_tts_node_routing_selects_onnx_runtime() {
    let (_mock_server, runtime_port) = start_mock_node_models_endpoint().await;
    let app = build_app().await;

    // TTS対応ノードを登録
    let register_payload = json!({
        "machine_name": "tts-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["onnx_runtime"],
        "loaded_tts_models": ["vibevoice-v1"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
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
            .map(|arr| arr.iter().any(|v| v.as_str() == Some("onnx_runtime")))
            .unwrap_or(false)
    });

    assert!(
        tts_node.is_some(),
        "A node with onnx_runtime capability should be registered"
    );
}

/// T015: 複合ノード選択テスト
///
/// 複数のRuntimeTypeを持つノードが適切に処理される
#[tokio::test]
async fn test_multi_runtime_node_handles_both_asr_and_tts() {
    let (_mock_server, runtime_port) = start_mock_node_models_endpoint().await;
    let app = build_app().await;

    // 複合ランタイム対応ノードを登録（ONNX + ASR）
    let register_payload = json!({
        "machine_name": "multi-runtime-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2, "memory": 24576}
        ],
        "supported_runtimes": ["onnx_runtime", "whisper_cpp"],
        "loaded_models": ["gpt-oss-20b"],
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
                .header("x-api-key", "sk_debug")
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

    // ノード一覧を確認して複数のランタイムが登録されていることを確認
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
    let multi_runtime_node = node_list
        .iter()
        .find(|n| n.get("id").and_then(|id| id.as_str()) == Some(node_id))
        .expect("registered node should appear in /api/nodes");

    let supported = multi_runtime_node
        .get("supported_runtimes")
        .and_then(|v| v.as_array())
        .expect("supported_runtimes should be an array");

    assert!(
        supported.iter().any(|v| v.as_str() == Some("onnx_runtime")),
        "A multi-runtime node should include onnx_runtime"
    );
    assert!(
        supported.iter().any(|v| v.as_str() == Some("whisper_cpp")),
        "A multi-runtime node should include whisper_cpp"
    );
}

/// T016: 対応ノードなしフォールバックテスト
///
/// 要求されたRuntimeTypeを持つノードがない場合、503を返す
#[tokio::test]
async fn test_no_capable_node_returns_503() {
    let (_mock_server, runtime_port) = start_mock_node_models_endpoint().await;
    let app = build_app().await;

    // ASRノードのみを登録（TTSなし）
    let register_payload = json!({
        "machine_name": "asr-only-node",
        "ip_address": "127.0.0.1",
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port,
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["whisper_cpp"],
        "loaded_asr_models": ["whisper-large-v3"]
    });

    let register_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nodes")
                .header("content-type", "application/json")
                .header("x-api-key", "sk_debug")
                .body(Body::from(serde_json::to_vec(&register_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // TTSリクエストを試行（JSON形式）: TTS対応ノードがないため503を期待
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
                .header("x-api-key", "sk_debug")
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
