//! 音声API統合テスト
//!
//! TDD RED: 音声認識（ASR）と音声合成（TTS）のノード選択とプロキシテスト

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use serde_json::json;
use tower::ServiceExt;

use crate::support::{admin::approve_node, http};

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
        endpoint_registry: None,
    };

    api::create_router(state)
}

async fn spawn_audio_stub() -> http::TestServerGuard {
    let router = Router::new()
        .route("/v1/audio/transcriptions", post(transcriptions_handler))
        .route("/v1/audio/speech", post(speech_handler))
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

async fn transcriptions_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "text": "ok"
        })),
    )
        .into_response()
}

async fn speech_handler() -> impl IntoResponse {
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/mpeg")
        .body(Body::from(vec![0_u8, 1, 2, 3]))
        .unwrap()
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": [
                {"id": "test-model", "object": "model"}
            ]
        })),
    )
        .into_response()
}

fn build_transcription_multipart(boundary: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: audio/wav\r\n\r\n");
    body.extend_from_slice(b"RIFF....DATA");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
    body.extend_from_slice(b"whisper-large-v3");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    body
}

/// T013: ASRノード選択テスト
///
/// RuntimeType.Whisperを持つノードが/v1/audio/transcriptionsにルーティングされる
#[tokio::test]
async fn test_asr_node_routing_selects_whisper_runtime() {
    let app = build_app().await;
    let stub = spawn_audio_stub().await;

    // Whisper対応ノードを登録
    let register_payload = json!({
        "machine_name": "whisper-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["whisper_cpp"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/internal/test/register-node")
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

    let boundary = "boundary-asr";
    let body = build_transcription_multipart(boundary);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/transcriptions")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .header("x-api-key", "sk_debug")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload.get("text").and_then(|v| v.as_str()), Some("ok"));
}

/// T014: TTSノード選択テスト
///
/// RuntimeType.OnnxTtsを持つノードが/v1/audio/speechにルーティングされる
#[tokio::test]
async fn test_tts_node_routing_selects_onnx_runtime() {
    let app = build_app().await;
    let stub = spawn_audio_stub().await;

    // TTS対応ノードを登録
    let register_payload = json!({
        "machine_name": "tts-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 1}
        ],
        "supported_runtimes": ["onnx_runtime"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/internal/test/register-node")
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

    let tts_request = json!({
        "model": "vibevoice-v1",
        "input": "テスト",
        "voice": "nova"
    });

    let response = app
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

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("audio/mpeg")
    );
}

/// T015: 複合ノード選択テスト
///
/// 複数のRuntimeTypeを持つノードが適切に処理される
#[tokio::test]
async fn test_multi_runtime_node_handles_both_asr_and_tts() {
    let app = build_app().await;
    let stub = spawn_audio_stub().await;

    // 複合ランタイム対応ノードを登録（LLM + ASR + TTS）
    let register_payload = json!({
        "machine_name": "multi-runtime-node",
        "ip_address": stub.addr().ip().to_string(),
        "runtime_version": "0.1.0",
        "runtime_port": runtime_port_for_stub(&stub),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "NVIDIA RTX 4090", "count": 2, "memory": 24576}
        ],
        "supported_runtimes": ["llama_cpp", "whisper_cpp", "onnx_runtime"]
    });

    let register_response = app
        .clone()
        .oneshot(
            node_register_request()
                .method("POST")
                .uri("/v0/internal/test/register-node")
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

    let boundary = "boundary-multi";
    let body = build_transcription_multipart(boundary);
    let asr_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/audio/transcriptions")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={}", boundary),
                )
                .header("x-api-key", "sk_debug")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(asr_response.status(), StatusCode::OK);

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
    assert_eq!(tts_response.status(), StatusCode::OK);
}

/// T016: 対応ノードなしフォールバックテスト
///
/// 要求されたRuntimeTypeを持つノードがない場合、503を返す
#[tokio::test]
async fn test_no_capable_node_returns_503() {
    let app = build_app().await;
    let stub = spawn_audio_stub().await;

    // LLMノードのみを登録（ASR/TTSなし）
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
                .uri("/v0/internal/test/register-node")
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
