//! Audio API error handling tests

#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{multipart, Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use serial_test::serial;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::spawn_test_lb,
};

#[derive(Clone)]
struct AsrStubState {
    response: AsrStubResponse,
}

#[derive(Clone)]
enum AsrStubResponse {
    Error(StatusCode, String),
}

async fn spawn_asr_stub(state: AsrStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/audio/transcriptions", post(asr_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn asr_handler(State(state): State<Arc<AsrStubState>>) -> impl IntoResponse {
    match &state.response {
        AsrStubResponse::Error(status, msg) => (*status, msg.clone()).into_response(),
    }
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({ "data": [{ "id": "whisper-large-v3" }] })),
    )
        .into_response()
}

async fn tags_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "models": [{ "name": "whisper-large-v3", "size": 3_000_000_000i64 }]
        })),
    )
        .into_response()
}

#[tokio::test]
#[serial]
async fn transcriptions_invalid_format_returns_400() {
    let asr_stub = spawn_asr_stub(AsrStubState {
        response: AsrStubResponse::Error(
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Unsupported audio format","type":"invalid_request_error"}}"#
                .to_string(),
        ),
    })
    .await;

    let lb = spawn_test_lb().await;

    // EndpointRegistry経由でエンドポイントを登録
    let client = Client::new();

    // 1. エンドポイントを作成（audio_transcription capability付き）
    let create_response = client
        .post(format!("http://{}/v0/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "ASR Test Endpoint",
            "base_url": format!("http://{}", asr_stub.addr()),
            "health_check_interval_secs": 30,
            "capabilities": ["audio_transcription"]
        }))
        .send()
        .await
        .expect("create endpoint should succeed");

    assert!(create_response.status().is_success());
    let create_body: Value = create_response.json().await.unwrap();
    let endpoint_id = create_body["id"].as_str().unwrap();

    // 2. エンドポイントをOnline状態にする
    let test_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("test endpoint should succeed");
    assert!(test_response.status().is_success());

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(vec![0x00, 0x01, 0x02, 0x03])
                .file_name("test.xyz")
                .mime_str("application/octet-stream")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = Client::new()
        .post(format!("http://{}/v1/audio/transcriptions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
#[serial]
async fn speech_empty_input_returns_400() {
    let lb = spawn_test_lb().await;

    let response = Client::new()
        .post(format!("http://{}/v1/audio/speech", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "vibevoice-v1",
            "input": "",
            "voice": "nova"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(body["error"]["message"], "Input text is required");
}

#[tokio::test]
#[serial]
async fn speech_input_too_long_returns_400() {
    let lb = spawn_test_lb().await;
    let long_input = "a".repeat(4097);

    let response = Client::new()
        .post(format!("http://{}/v1/audio/speech", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "vibevoice-v1",
            "input": long_input,
            "voice": "nova"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(
        body["error"]["message"],
        "Input text exceeds maximum length of 4096 characters"
    );
}
