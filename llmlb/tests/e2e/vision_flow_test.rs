//! E2E RED: 画像認識（Vision Chat）
//!
//! 実装前の期待振る舞いを定義する（ignored）。

use crate::support;
use axum::{
    body::Body,
    http::header,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine};
use llmlb::{
    common::types::ModelCapability, db::models::ModelStorage, registry::models::ModelInfo,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;

const VISION_MODEL_ID: &str = "vision-model";

async fn spawn_vision_stub() -> support::http::TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/health", get(|| async { StatusCode::OK }))
        .route("/test-image.png", get(test_image_handler));
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
                { "id": VISION_MODEL_ID }
            ]
        })),
    )
        .into_response()
}

async fn test_image_handler() -> impl IntoResponse {
    let bytes = general_purpose::STANDARD
        .decode(support::images::TEST_IMAGE_1X1_TRANSPARENT_PNG)
        .expect("decode test image");
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(bytes))
        .expect("build image response")
}

async fn register_models(db_pool: &SqlitePool) {
    let storage = ModelStorage::new(db_pool.clone());
    let vision_model = ModelInfo::with_capabilities(
        VISION_MODEL_ID.to_string(),
        0,
        "vision test model".to_string(),
        0,
        vec![],
        vec![ModelCapability::TextGeneration, ModelCapability::Vision],
    );
    storage
        .save_model(&vision_model)
        .await
        .expect("save vision model");
}

async fn register_and_sync_endpoint(
    lb: &support::http::TestServer,
    node: &support::http::TestServer,
) {
    let endpoint_id = support::lb::register_endpoint_with_capabilities(
        lb.addr(),
        node.addr(),
        "Vision Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint should succeed");

    let response = Client::new()
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("sync should succeed");

    assert!(
        response.status().is_success(),
        "sync should return success (status: {})",
        response.status()
    );
}

async fn setup_lb_with_node() -> (support::http::TestServer, support::http::TestServer) {
    let node = spawn_vision_stub().await;
    let (lb, db_pool) = support::lb::spawn_test_lb_with_db().await;
    register_models(&db_pool).await;
    register_and_sync_endpoint(&lb, &node).await;
    (lb, node)
}

#[tokio::test]
async fn e2e_vision_chat_with_image_url_returns_text() {
    let (lb, node) = setup_lb_with_node().await;
    let image_url = format!("http://{}/test-image.png", node.addr());

    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": VISION_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": "What is in this image?" },
                        { "type": "image_url", "image_url": { "url": image_url } }
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
async fn e2e_vision_chat_with_multiple_images_returns_text() {
    let (lb, node) = setup_lb_with_node().await;
    let image_url = format!("http://{}/test-image.png", node.addr());

    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": VISION_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": "Compare these images." },
                        { "type": "image_url", "image_url": { "url": image_url } },
                        { "type": "image_url", "image_url": { "url": image_url } }
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
