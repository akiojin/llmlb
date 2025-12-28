//! E2E RED: 音声API（ASR/TTS）
//!
//! 実装前の期待振る舞いを定義する（ignored）。

use crate::support;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{multipart, Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};

async fn spawn_audio_stub() -> support::http::TestServer {
    let router = Router::new()
        .route("/v1/audio/transcriptions", post(transcriptions_handler))
        .route("/v1/audio/speech", post(speech_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/health", post(|| async { StatusCode::OK }));
    support::http::spawn_router(router).await
}

async fn transcriptions_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "text": "hello" }))).into_response()
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
                { "id": "whisper-large-v3" },
                { "id": "tts-1" }
            ]
        })),
    )
        .into_response()
}

async fn register_audio_node(router: &support::http::TestServer, node: &support::http::TestServer) {
    let response = support::router::register_node_with_runtimes(
        router.addr(),
        node.addr(),
        vec!["whisper_cpp", "onnx_runtime"],
    )
    .await
    .expect("register node should succeed");
    let (status, _body) =
        support::router::approve_node_from_register_response(router.addr(), response)
            .await
            .expect("approve node should succeed");
    assert_eq!(status, ReqStatusCode::CREATED);
}

fn build_dummy_wav() -> Vec<u8> {
    vec![
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // chunk size
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // subchunk1 size
        0x01, 0x00, // PCM
        0x01, 0x00, // channels
        0x22, 0x56, 0x00, 0x00, // sample rate
        0x44, 0xac, 0x00, 0x00, // byte rate
        0x02, 0x00, // block align
        0x10, 0x00, // bits per sample
        0x64, 0x61, 0x74, 0x61, // "data"
        0x00, 0x00, 0x00, 0x00, // data size
    ]
}

#[tokio::test]
#[ignore = "TDD RED: ASR E2E not yet implemented"]
async fn e2e_audio_transcriptions_returns_text() {
    let node = spawn_audio_stub().await;
    let router = support::router::spawn_test_router().await;
    register_audio_node(&router, &node).await;

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(build_dummy_wav())
                .file_name("test.wav")
                .mime_str("audio/wav")
                .unwrap(),
        )
        .text("model", "whisper-large-v3");

    let response = Client::new()
        .post(format!("http://{}/v1/audio/transcriptions", router.addr()))
        .header("x-api-key", "sk_debug")
        .multipart(form)
        .send()
        .await
        .expect("transcriptions request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    assert!(
        payload["text"].as_str().unwrap_or_default().len() > 0,
        "expected non-empty transcription text"
    );

    node.stop().await;
    router.stop().await;
}

#[tokio::test]
#[ignore = "TDD RED: TTS E2E not yet implemented"]
async fn e2e_audio_speech_returns_audio() {
    let node = spawn_audio_stub().await;
    let router = support::router::spawn_test_router().await;
    register_audio_node(&router, &node).await;

    let response = Client::new()
        .post(format!("http://{}/v1/audio/speech", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "tts-1",
            "input": "こんにちは",
            "voice": "alloy",
            "format": "mp3"
        }))
        .send()
        .await
        .expect("speech request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok()),
        Some("audio/mpeg")
    );
    let bytes = response.bytes().await.expect("audio bytes");
    assert!(!bytes.is_empty(), "expected audio payload");

    node.stop().await;
    router.stop().await;
}
