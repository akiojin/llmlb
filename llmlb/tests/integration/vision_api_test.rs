//! Integration Test: Vision API End-to-End (SPEC-e03a404c)
//!
//! LB経由でのVisionフローをモックノードで検証する。

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use serial_test::serial;
use sqlx::SqlitePool;

use crate::support::{
    http::{spawn_lb, TestServer},
    images,
    lb::{register_endpoint_with_capabilities, spawn_test_lb_with_db},
};
use llmlb::{
    common::types::ModelCapability, db::models::ModelStorage, registry::models::ModelInfo,
};

const VISION_MODEL_ID: &str = "llava-v1.5-7b";
const TEXT_MODEL_ID: &str = "llama-3.1-8b";

#[derive(Clone)]
struct VisionNodeState {
    model_ids: Vec<String>,
}

/// Vision対応モックノード: /v1/chat/completions
async fn vision_chat_handler(
    State(state): State<Arc<VisionNodeState>>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    let model = req["model"].as_str().unwrap_or("");
    if !state.model_ids.iter().any(|id| id == model) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "message": format!("Model '{}' not found", model),
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    // メッセージに画像が含まれているかチェック
    let has_image = req["messages"]
        .as_array()
        .map(|msgs| {
            msgs.iter().any(|msg| {
                msg["content"]
                    .as_array()
                    .map(|content| {
                        content
                            .iter()
                            .any(|c| c["type"].as_str() == Some("image_url"))
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    // 成功レスポンス
    let response_text = if has_image {
        "I can see an image. This appears to be a test image."
    } else {
        "Hello! How can I help you?"
    };

    (
        StatusCode::OK,
        Json(json!({
            "id": "chatcmpl-vision-test",
            "object": "chat.completion",
            "created": 1234567890,
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": response_text
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 20,
                "total_tokens": 120
            }
        })),
    )
        .into_response()
}

/// /v1/models エンドポイント
async fn models_handler(State(state): State<Arc<VisionNodeState>>) -> impl IntoResponse {
    let data: Vec<Value> = state
        .model_ids
        .iter()
        .map(|model_id| {
            let capabilities = if model_id == VISION_MODEL_ID {
                json!({"image_understanding": true})
            } else {
                json!({})
            };
            json!({
                "id": model_id,
                "object": "model",
                "created": 1234567890,
                "owned_by": "test",
                "capabilities": capabilities
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": data
        })),
    )
        .into_response()
}

async fn health_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok"
        })),
    )
        .into_response()
}

async fn test_image_handler() -> impl IntoResponse {
    let bytes = general_purpose::STANDARD
        .decode(images::TEST_IMAGE_1X1_TRANSPARENT_PNG)
        .expect("decode test image");
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .body(Body::from(bytes))
        .expect("build image response")
}

async fn spawn_vision_node(model_ids: &[&str]) -> TestServer {
    let state = Arc::new(VisionNodeState {
        model_ids: model_ids.iter().map(|id| id.to_string()).collect(),
    });

    let app = Router::new()
        .route("/v1/chat/completions", post(vision_chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/v0/health", get(health_handler))
        .route("/test-image.png", get(test_image_handler))
        .with_state(state);

    spawn_lb(app).await
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
    let text_model = ModelInfo::new(
        TEXT_MODEL_ID.to_string(),
        0,
        "text test model".to_string(),
        0,
        vec![],
    );
    storage
        .save_model(&vision_model)
        .await
        .expect("save vision model");
    storage
        .save_model(&text_model)
        .await
        .expect("save text model");
}

async fn register_and_sync_endpoint(lb: &TestServer, node: &TestServer) -> String {
    let endpoint_id = register_endpoint_with_capabilities(
        lb.addr(),
        node.addr(),
        "Vision Endpoint",
        &["chat_completion"],
    )
    .await
    .expect("register endpoint should succeed");

    let response = Client::new()
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("sync should succeed");

    assert!(
        response.status().is_success(),
        "sync should return success (status: {})",
        response.status()
    );

    endpoint_id
}

async fn setup_lb_with_node() -> (TestServer, TestServer) {
    let node = spawn_vision_node(&[VISION_MODEL_ID, TEXT_MODEL_ID]).await;
    let (lb, db_pool) = spawn_test_lb_with_db().await;
    register_models(&db_pool).await;
    let _ = register_and_sync_endpoint(&lb, &node).await;
    (lb, node)
}

/// US1: 画像URL付きチャットリクエストの正常処理
#[tokio::test]
#[serial]
async fn test_vision_chat_with_image_url_integration() {
    let (lb, node) = setup_lb_with_node().await;
    let image_url = format!("http://{}/test-image.png", node.addr());

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": VISION_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "What is in this image?"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": image_url
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 300
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    assert!(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .contains("image"));

    node.stop().await;
    lb.stop().await;
}

/// US1: Base64画像付きリクエストの正常処理
#[tokio::test]
#[serial]
async fn test_vision_chat_with_base64_image_integration() {
    let (lb, node) = setup_lb_with_node().await;
    let tiny_png_base64 = images::test_image_tiny_data_uri();

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": VISION_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Describe this image"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": tiny_png_base64
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 300
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    assert!(body["choices"][0]["message"]["content"].as_str().is_some());

    node.stop().await;
    lb.stop().await;
}

/// US2: Vision非対応モデルへのリクエストエラー
#[tokio::test]
#[serial]
async fn test_vision_request_to_text_only_model_integration() {
    let (lb, node) = setup_lb_with_node().await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": TEXT_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "What is in this image?"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "https://example.com/test-image.jpg"
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 300
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("valid json");
    let error_msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("image") || error_msg.contains("vision"),
        "Error should mention image/vision: {}",
        error_msg
    );

    node.stop().await;
    lb.stop().await;
}

/// US4: /v1/models でVision capabilityを確認
#[tokio::test]
#[serial]
async fn test_models_endpoint_shows_vision_capability_integration() {
    let (lb, node) = setup_lb_with_node().await;

    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    let models = body["data"].as_array().expect("models array should exist");
    let vision_model = models
        .iter()
        .find(|m| m["id"].as_str() == Some(VISION_MODEL_ID))
        .expect("vision model must exist");
    let text_model = models
        .iter()
        .find(|m| m["id"].as_str() == Some(TEXT_MODEL_ID))
        .expect("text model must exist");

    assert_eq!(
        vision_model["capabilities"]["image_understanding"],
        json!(true)
    );
    let has_vision = text_model["capabilities"]["image_understanding"]
        .as_bool()
        .unwrap_or(false);
    assert!(!has_vision);

    node.stop().await;
    lb.stop().await;
}

/// パフォーマンス: 1024x1024画像の処理が5秒以内に完了する
/// TDD RED: Vision機能が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision performance test requires implementation"]
async fn test_vision_processing_performance() {
    let node = spawn_vision_node(&[VISION_MODEL_ID]).await;

    let client = Client::new();
    let start = std::time::Instant::now();

    let response = client
        .post(format!("http://{}/v1/chat/completions", node.addr()))
        .json(&json!({
            "model": VISION_MODEL_ID,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Describe this image in detail"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "https://example.com/1024x1024-image.jpg"
                            }
                        }
                    ]
                }
            ],
            "max_tokens": 500
        }))
        .send()
        .await
        .expect("request should succeed");

    let elapsed = start.elapsed();

    assert_eq!(response.status(), ReqStatusCode::OK);
    assert!(
        elapsed.as_secs() < 5,
        "Vision processing should complete within 5 seconds (took {:?})",
        elapsed
    );

    node.stop().await;
}
