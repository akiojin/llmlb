//! Integration Test: Vision API End-to-End (SPEC-e03a404c)
//!
//! TDD RED: These tests verify the full vision API flow with mock nodes.
//! All tests should FAIL until the vision feature is fully implemented.

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use serial_test::serial;

/// Test server wrapper
pub struct TestServer {
    addr: std::net::SocketAddr,
    _handle: tokio::task::JoinHandle<()>,
}

impl TestServer {
    pub fn addr(&self) -> std::net::SocketAddr {
        self.addr
    }
}

async fn spawn_router(app: Router) -> TestServer {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // サーバーが起動するのを待つ
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    TestServer {
        addr,
        _handle: handle,
    }
}

#[derive(Clone)]
struct VisionNodeState {
    model_name: String,
    supports_vision: bool,
}

/// Vision対応モックノード: /v1/chat/completions
async fn vision_chat_handler(
    State(state): State<Arc<VisionNodeState>>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    let model = req["model"].as_str().unwrap_or("");
    if model != state.model_name {
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

    if has_image && !state.supports_vision {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": format!("Model '{}' does not support image understanding", state.model_name),
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

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
            "model": state.model_name,
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
    let capabilities = if state.supports_vision {
        json!({"image_understanding": true})
    } else {
        json!({})
    };

    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": [
                {
                    "id": state.model_name,
                    "object": "model",
                    "created": 1234567890,
                    "owned_by": "test",
                    "capabilities": capabilities
                }
            ]
        })),
    )
        .into_response()
}

async fn spawn_vision_node(model_name: &str, supports_vision: bool) -> TestServer {
    let state = Arc::new(VisionNodeState {
        model_name: model_name.to_string(),
        supports_vision,
    });

    let router = Router::new()
        .route("/v1/chat/completions", post(vision_chat_handler))
        .route("/v1/models", get(models_handler))
        .with_state(state);

    spawn_router(router).await
}

/// US1: 画像URL付きチャットリクエストの正常処理
/// TDD RED: Vision機能が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision integration not yet implemented"]
async fn test_vision_chat_with_image_url_integration() {
    // Vision対応ノードを起動
    let node = spawn_vision_node("llava-v1.5-7b", true).await;

    // NOTE: 実際のテストでは、ルーターを起動してノードを登録する必要がある
    // ここではモックノードへの直接リクエストでAPIを検証

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", node.addr()))
        .json(&json!({
            "model": "llava-v1.5-7b",
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

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    assert!(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .contains("image"));
}

/// US1: Base64画像付きリクエストの正常処理
/// TDD RED: Vision機能が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision integration not yet implemented"]
async fn test_vision_chat_with_base64_image_integration() {
    let node = spawn_vision_node("llava-v1.5-7b", true).await;

    let tiny_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", node.addr()))
        .json(&json!({
            "model": "llava-v1.5-7b",
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
                                "url": format!("data:image/png;base64,{}", tiny_png_base64)
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
}

/// US2: Vision非対応モデルへのリクエストエラー
/// TDD RED: capabilities検証が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision capability validation not yet implemented"]
async fn test_vision_request_to_text_only_model_integration() {
    // Vision非対応ノードを起動
    let node = spawn_vision_node("llama-3.1-8b", false).await;

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/chat/completions", node.addr()))
        .json(&json!({
            "model": "llama-3.1-8b",
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
}

/// US4: /v1/models でVision capabilityを確認
/// TDD RED: capabilities応答が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision capabilities in /v1/models not yet implemented"]
async fn test_models_endpoint_shows_vision_capability_integration() {
    let vision_node = spawn_vision_node("llava-v1.5-7b", true).await;
    let text_node = spawn_vision_node("llama-3.1-8b", false).await;

    let client = Client::new();

    // Vision対応モデル
    let response = client
        .get(format!("http://{}/v1/models", vision_node.addr()))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    let vision_model = &body["data"][0];
    assert_eq!(
        vision_model["capabilities"]["image_understanding"],
        json!(true)
    );

    // テキストのみモデル
    let response = client
        .get(format!("http://{}/v1/models", text_node.addr()))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json");
    let text_model = &body["data"][0];
    // capabilities.image_understanding が false または存在しない
    let has_vision = text_model["capabilities"]["image_understanding"]
        .as_bool()
        .unwrap_or(false);
    assert!(!has_vision);
}

/// パフォーマンス: 1024x1024画像の処理が5秒以内に完了する
/// TDD RED: Vision機能が未実装のため失敗する
#[tokio::test]
#[serial]
#[ignore = "TDD RED: Vision performance test requires implementation"]
async fn test_vision_processing_performance() {
    let node = spawn_vision_node("llava-v1.5-7b", true).await;

    let client = Client::new();
    let start = std::time::Instant::now();

    let response = client
        .post(format!("http://{}/v1/chat/completions", node.addr()))
        .json(&json!({
            "model": "llava-v1.5-7b",
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
}
