//! Additional Contract Tests: Audio APIs (transcription + speech)
//!
//! 既存テストと重複しない追加ケース

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_audio_speech_endpoint, register_audio_transcription_endpoint, spawn_test_lb},
};
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{multipart, Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;

// --- TTS Stubs ---

#[derive(Clone)]
struct TtsStubState {
    expected_model: Option<String>,
    audio_data: Vec<u8>,
}

async fn spawn_tts_stub(state: TtsStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/audio/speech", post(tts_handler))
        .route("/v1/models", get(tts_models_handler))
        .route("/api/tags", get(tts_tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn tts_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    let body = Body::from(state.audio_data.clone());
    (StatusCode::OK, [(header::CONTENT_TYPE, "audio/mpeg")], body).into_response()
}

async fn tts_models_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    let model_name = state
        .expected_model
        .as_deref()
        .unwrap_or("vibevoice-v1");
    (
        StatusCode::OK,
        Json(serde_json::json!({"data": [{"id": model_name}]})),
    )
        .into_response()
}

async fn tts_tags_handler(State(state): State<Arc<TtsStubState>>) -> impl IntoResponse {
    let model_name = state
        .expected_model
        .as_deref()
        .unwrap_or("vibevoice-v1");
    (
        StatusCode::OK,
        Json(serde_json::json!({"models": [{"name": model_name, "size": 500_000_000i64}]})),
    )
        .into_response()
}

// --- ASR Stubs ---

#[derive(Clone)]
struct AsrStubState {
    expected_model: Option<String>,
    response_text: String,
}

async fn spawn_asr_stub(state: AsrStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/audio/transcriptions", post(asr_handler))
        .route("/v1/models", get(asr_models_handler))
        .route("/api/tags", get(asr_tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn asr_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({"text": state.response_text})),
    )
        .into_response()
}

async fn asr_models_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    let model_name = state
        .expected_model
        .as_deref()
        .unwrap_or("whisper-large-v3");
    (
        StatusCode::OK,
        Json(serde_json::json!({"data": [{"id": model_name}]})),
    )
        .into_response()
}

async fn asr_tags_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    let model_name = state
        .expected_model
        .as_deref()
        .unwrap_or("whisper-large-v3");
    (
        StatusCode::OK,
        Json(serde_json::json!({"models": [{"name": model_name, "size": 3_000_000_000i64}]})),
    )
        .into_response()
}

// =============================================================================
// Additional TTS Tests
// =============================================================================

/// TTS: モデルなしリクエスト → 400 or 422
#[tokio::test]
#[serial]
async fn speech_missing_model_returns_error() {
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "input": "テスト"
        }))
        .send()
        .await
        .expect("request should complete");

    // model missing should be rejected
    let status = response.status();
    assert!(
        status == ReqStatusCode::BAD_REQUEST || status == ReqStatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {}",
        status
    );
}

/// TTS: 無効なAPIキー → 401
#[tokio::test]
#[serial]
async fn speech_invalid_api_key_returns_401() {
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "invalid_key_12345")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": "テスト"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

/// TTS: レスポンスボディが空でないことを確認（大きなテキスト入力）
#[tokio::test]
#[serial]
async fn speech_large_input_within_limit() {
    let dummy_mp3 = vec![0x49, 0x44, 0x33, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

    let tts_stub = spawn_tts_stub(TtsStubState {
        expected_model: Some("vibevoice-v1".to_string()),
        audio_data: dummy_mp3,
    })
    .await;

    let coordinator = spawn_test_lb().await;
    let _endpoint_id = register_audio_speech_endpoint(coordinator.addr(), tts_stub.addr())
        .await
        .expect("register endpoint must succeed");

    // 4096文字以内のテキスト
    let input_text = "あ".repeat(4096);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "vibevoice-v1",
            "input": input_text
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body = response.bytes().await.unwrap();
    assert!(!body.is_empty());
}

/// TTS: GETメソッドは405
#[tokio::test]
#[serial]
async fn speech_get_method_not_allowed() {
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/audio/speech", coordinator.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::METHOD_NOT_ALLOWED);
}

// =============================================================================
// Additional Transcription Tests
// =============================================================================

/// Transcription: 言語パラメータ付きリクエスト
#[tokio::test]
#[serial]
async fn transcriptions_with_language_param() {
    let asr_stub = spawn_asr_stub(AsrStubState {
        expected_model: Some("whisper-large-v3".to_string()),
        response_text: "Hello world".to_string(),
    })
    .await;

    let coordinator = spawn_test_lb().await;
    let _endpoint_id =
        register_audio_transcription_endpoint(coordinator.addr(), asr_stub.addr())
            .await
            .expect("register endpoint must succeed");

    let dummy_wav = vec![
        0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6d,
        0x74, 0x20, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x22, 0x56, 0x00, 0x00,
        0x44, 0xAC, 0x00, 0x00, 0x02, 0x00, 0x10, 0x00, 0x64, 0x61, 0x74, 0x61, 0x00, 0x00,
        0x00, 0x00,
    ];

    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(dummy_wav)
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3")
        .text("language", "en");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("transcription request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");
    assert!(body["text"].is_string());
}

/// Transcription: 無効なAPIキー → 401
#[tokio::test]
#[serial]
async fn transcriptions_invalid_api_key_returns_401() {
    let coordinator = spawn_test_lb().await;

    let dummy_wav = vec![0x52, 0x49, 0x46, 0x46];
    let client = Client::new();
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(dummy_wav)
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "invalid_key_12345")
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

/// Transcription: GETメソッドは405
#[tokio::test]
#[serial]
async fn transcriptions_get_method_not_allowed() {
    let coordinator = spawn_test_lb().await;

    let client = Client::new();
    let response = client
        .get(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::METHOD_NOT_ALLOWED);
}

/// Transcription: ファイルなしのリクエスト → 400
#[tokio::test]
#[serial]
async fn transcriptions_missing_file_returns_400() {
    let asr_stub = spawn_asr_stub(AsrStubState {
        expected_model: None,
        response_text: String::new(),
    })
    .await;

    let coordinator = spawn_test_lb().await;
    let _endpoint_id =
        register_audio_transcription_endpoint(coordinator.addr(), asr_stub.addr())
            .await
            .expect("register endpoint must succeed");

    let client = Client::new();
    let form = multipart::Form::new().text("model", "whisper-large-v3");
    // file field missing

    let response = client
        .post(format!(
            "http://{}/v1/audio/transcriptions",
            coordinator.addr()
        ))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
}
