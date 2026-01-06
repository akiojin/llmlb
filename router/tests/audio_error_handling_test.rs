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
    http::{spawn_router, TestServer},
    router::{approve_node_from_register_response, register_node_with_runtimes, spawn_test_router},
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
    let router = Router::new()
        .route("/v1/audio/transcriptions", post(asr_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_router(router).await
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

    let router = spawn_test_router().await;

    let register_response =
        register_node_with_runtimes(router.addr(), asr_stub.addr(), vec!["whisper_cpp"])
            .await
            .expect("register node should succeed");

    let (status, _body) = approve_node_from_register_response(router.addr(), register_response)
        .await
        .expect("approve node should succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

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
        .post(format!("http://{}/v1/audio/transcriptions", router.addr()))
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
    let router = spawn_test_router().await;

    let response = Client::new()
        .post(format!("http://{}/v1/audio/speech", router.addr()))
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
    let router = spawn_test_router().await;
    let long_input = "a".repeat(4097);

    let response = Client::new()
        .post(format!("http://{}/v1/audio/speech", router.addr()))
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
