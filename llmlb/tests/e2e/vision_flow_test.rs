//! E2E RED: 画像認識（Vision Chat）
//!
//! 実装前の期待振る舞いを定義する（ignored）。

use crate::support;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};

async fn spawn_vision_stub() -> support::http::TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/health", post(|| async { StatusCode::OK }));
    support::http::spawn_lb(app).await
}

async fn chat_handler(Json(_req): Json<Value>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "id": "chatcmpl-vision-e2e",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "vision-model",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "This is a test response for vision."
                    }
                }
            ]
        })),
    )
        .into_response()
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": [
                { "id": "vision-model" }
            ]
        })),
    )
        .into_response()
}

async fn register_vision_endpoint(
    lb: &support::http::TestServer,
    node: &support::http::TestServer,
) -> String {
    let client = Client::new();

    let register_response = client
        .post(format!("http://{}/v0/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "vision-stub",
            "base_url": format!("http://{}", node.addr())
        }))
        .send()
        .await
        .expect("endpoint registration should succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let register_body: Value = register_response
        .json()
        .await
        .expect("endpoint registration response must be json");
    let endpoint_id = register_body["id"]
        .as_str()
        .expect("endpoint id should exist")
        .to_string();

    let test_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test should succeed");
    assert_eq!(test_response.status(), ReqStatusCode::OK);

    let sync_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint sync should succeed");
    assert_eq!(sync_response.status(), ReqStatusCode::OK);

    endpoint_id
}

#[tokio::test]
#[ignore = "TDD RED: vision E2E not yet implemented"]
async fn e2e_vision_chat_with_image_url_returns_text() {
    let node = spawn_vision_stub().await;
    let lb = support::lb::spawn_test_lb().await;
    let _endpoint_id = register_vision_endpoint(&lb, &node).await;

    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "vision-model",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": "What is in this image?" },
                        { "type": "image_url", "image_url": { "url": "https://example.com/test.jpg" } }
                    ]
                }
            ],
            "max_tokens": 128
        }))
        .send()
        .await
        .expect("vision chat request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    let content = payload["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default();
    assert!(!content.is_empty(), "expected non-empty vision response");

    node.stop().await;
    lb.stop().await;
}

#[tokio::test]
#[ignore = "TDD RED: vision multi-image E2E not yet implemented"]
async fn e2e_vision_chat_with_multiple_images_returns_text() {
    let node = spawn_vision_stub().await;
    let lb = support::lb::spawn_test_lb().await;
    let _endpoint_id = register_vision_endpoint(&lb, &node).await;

    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "vision-model",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": "Compare these images." },
                        { "type": "image_url", "image_url": { "url": "https://example.com/one.jpg" } },
                        { "type": "image_url", "image_url": { "url": "https://example.com/two.jpg" } }
                    ]
                }
            ],
            "max_tokens": 128
        }))
        .send()
        .await
        .expect("vision chat request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let payload: Value = response.json().await.expect("json response");
    let content = payload["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default();
    assert!(!content.is_empty(), "expected non-empty vision response");

    node.stop().await;
    lb.stop().await;
}
