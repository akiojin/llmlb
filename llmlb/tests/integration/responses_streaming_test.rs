//! Integration Test: Open Responses API Streaming
//!
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! T061: ストリーミングテスト

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_responses_endpoint, spawn_test_lb},
};
use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;

#[derive(Clone)]
struct StreamingNodeState {
    model_id: String,
}

/// ストリーミング対応のモックノードを起動
async fn spawn_streaming_node(state: StreamingNodeState) -> TestServer {
    let app = Router::new()
        .route("/v1/responses", post(streaming_responses_handler))
        .route("/v1/models", get(models_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn streaming_responses_handler(
    State(state): State<Arc<StreamingNodeState>>,
    Json(req): Json<Value>,
) -> impl axum::response::IntoResponse {
    let is_streaming = req["stream"].as_bool().unwrap_or(false);

    if !is_streaming {
        // 非ストリーミングレスポンス
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": "resp_non_stream",
                "object": "response",
                "model": state.model_id
            })),
        )
            .into_response();
    }

    // ストリーミングレスポンス（Server-Sent Events形式）
    let model_id = state.model_id.clone();

    // Open Responses API のストリーミングイベント形式に従う
    let events = vec![
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.created",
                "response": {
                    "id": "resp_streaming_test",
                    "object": "response",
                    "model": model_id
                }
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "message",
                    "role": "assistant"
                }
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.content_part.added",
                "part": {
                    "type": "text",
                    "text": ""
                }
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.output_text.delta",
                "delta": "Hello"
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.output_text.delta",
                "delta": " from"
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.output_text.delta",
                "delta": " streaming!"
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.output_text.done",
                "text": "Hello from streaming!"
            })
        ),
        format!(
            "data: {}\n\n",
            serde_json::json!({
                "type": "response.done",
                "response": {
                    "id": "resp_streaming_test",
                    "object": "response",
                    "status": "completed"
                }
            })
        ),
        "data: [DONE]\n\n".to_string(),
    ];

    // 全イベントを結合してレスポンス
    let body = events.join("");

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(Body::from(body))
        .unwrap()
        .into_response()
}

async fn models_handler(State(state): State<Arc<StreamingNodeState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "object": "list",
            "data": [{
                "id": state.model_id,
                "object": "model",
                "supported_apis": ["chat_completions", "responses"]
            }]
        })),
    )
        .into_response()
}

// =============================================================================
// T061: ストリーミングテスト
// =============================================================================

#[tokio::test]
#[serial]
async fn responses_streaming_passthrough_events() {
    // ストリーミング対応ノードを起動
    let node_stub = spawn_streaming_node(StreamingNodeState {
        model_id: "streaming-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "streaming-model")
        .await
        .expect("register endpoint must succeed");

    // ストリーミングリクエストを送信
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "streaming-model",
            "input": "Hello!",
            "stream": true
        }))
        .send()
        .await
        .expect("streaming responses request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);

    // Content-Typeがtext/event-streamであることを確認
    let content_type = response
        .headers()
        .get("content-type")
        .expect("should have content-type header")
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/event-stream"),
        "content-type should be text/event-stream"
    );

    // ストリーミングイベントを収集
    let mut event_count = 0;
    let mut received_done = false;
    let mut bytes = response.bytes_stream();

    while let Some(chunk_result) = bytes.next().await {
        let chunk = chunk_result.expect("should receive chunk");
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") {
                event_count += 1;
                if line.contains("[DONE]") {
                    received_done = true;
                }
            }
        }
    }

    // 複数のイベントを受信していることを確認
    assert!(event_count > 0, "should receive streaming events");
    assert!(received_done, "should receive [DONE] event");
}

#[tokio::test]
#[serial]
async fn responses_streaming_events_preserve_order() {
    // ストリーミング対応ノードを起動
    let node_stub = spawn_streaming_node(StreamingNodeState {
        model_id: "order-test-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "order-test-model")
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "order-test-model",
            "input": "Test",
            "stream": true
        }))
        .send()
        .await
        .expect("streaming request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);

    // イベントタイプの順序を確認
    let mut event_types: Vec<String> = Vec::new();
    let mut bytes = response.bytes_stream();

    while let Some(chunk_result) = bytes.next().await {
        let chunk = chunk_result.expect("should receive chunk");
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") && !line.contains("[DONE]") {
                let data = line.strip_prefix("data: ").unwrap();
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if let Some(event_type) = json["type"].as_str() {
                        event_types.push(event_type.to_string());
                    }
                }
            }
        }
    }

    // 最初のイベントはresponse.createdであるべき
    assert!(
        !event_types.is_empty(),
        "should have received at least one event"
    );
    assert_eq!(
        event_types.first().map(|s| s.as_str()),
        Some("response.created"),
        "first event should be response.created"
    );

    // 最後のイベント（[DONE]除く）はresponse.doneであるべき
    assert_eq!(
        event_types.last().map(|s| s.as_str()),
        Some("response.done"),
        "last event should be response.done"
    );
}

#[tokio::test]
#[serial]
async fn responses_streaming_collects_full_text() {
    // ストリーミング対応ノードを起動
    let node_stub = spawn_streaming_node(StreamingNodeState {
        model_id: "text-collect-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "text-collect-model")
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "text-collect-model",
            "input": "Collect text",
            "stream": true
        }))
        .send()
        .await
        .expect("streaming request should succeed");

    // テキストデルタを収集
    let mut collected_text = String::new();
    let mut bytes = response.bytes_stream();

    while let Some(chunk_result) = bytes.next().await {
        let chunk = chunk_result.expect("should receive chunk");
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if line.starts_with("data: ") && !line.contains("[DONE]") {
                let data = line.strip_prefix("data: ").unwrap();
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    if json["type"] == "response.output_text.delta" {
                        if let Some(delta) = json["delta"].as_str() {
                            collected_text.push_str(delta);
                        }
                    }
                }
            }
        }
    }

    // 収集したテキストが正しいことを確認
    assert_eq!(
        collected_text, "Hello from streaming!",
        "collected text should match expected output"
    );
}
