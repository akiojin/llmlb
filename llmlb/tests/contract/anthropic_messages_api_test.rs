//! Contract tests for the Anthropic-native Messages API (`/v1/messages`).

use crate::support::{
    http::TestServer,
    lb::{register_responses_endpoint, spawn_test_lb},
};
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
use std::sync::Arc;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Clone)]
struct ChatNodeStubState {
    response: ChatStubResponse,
}

#[derive(Clone)]
enum ChatStubResponse {
    Json(Value),
    Stream(String),
}

async fn spawn_chat_node_stub(state: ChatNodeStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .with_state(Arc::new(state));
    crate::support::http::spawn_lb(app).await
}

async fn chat_handler(
    State(state): State<Arc<ChatNodeStubState>>,
    Json(_request): Json<Value>,
) -> impl IntoResponse {
    match &state.response {
        ChatStubResponse::Json(payload) => (StatusCode::OK, Json(payload.clone())).into_response(),
        ChatStubResponse::Stream(body) => axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(axum::body::Body::from(body.clone()))
            .expect("stream response should build"),
    }
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "data": [
                {"id": "test-model", "object": "model"}
            ]
        })),
    )
}

#[tokio::test]
#[serial]
async fn anthropic_messages_local_request_success() {
    let node = spawn_chat_node_stub(ChatNodeStubState {
        response: ChatStubResponse::Json(json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "model": "test-model",
            "choices": [
                {
                    "message": {
                        "role": "assistant",
                        "content": "Hello from local endpoint"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 8,
                "completion_tokens": 5,
                "total_tokens": 13
            }
        })),
    })
    .await;
    let lb = spawn_test_lb().await;
    let _ = register_responses_endpoint(lb.addr(), node.addr(), "test-model")
        .await
        .expect("endpoint registration should succeed");

    let response = Client::new()
        .post(format!("http://{}/v1/messages", lb.addr()))
        .header("x-api-key", "sk_debug")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "test-model",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("response must be json");
    assert_eq!(body["type"], "message");
    assert_eq!(body["role"], "assistant");
    assert_eq!(body["content"][0]["text"], "Hello from local endpoint");
    assert_eq!(body["usage"]["input_tokens"], 8);
    assert_eq!(body["usage"]["output_tokens"], 5);
}

#[tokio::test]
#[serial]
async fn anthropic_messages_streaming_transforms_openai_sse() {
    let node = spawn_chat_node_stub(ChatNodeStubState {
        response: ChatStubResponse::Stream(concat!(
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"role\":\"assistant\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"},\"index\":0}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\",\"index\":0}]}\n\n",
            "data: [DONE]\n\n"
        )
        .to_string()),
    })
    .await;
    let lb = spawn_test_lb().await;
    let _ = register_responses_endpoint(lb.addr(), node.addr(), "test-model")
        .await
        .expect("endpoint registration should succeed");

    let response = Client::new()
        .post(format!("http://{}/v1/messages", lb.addr()))
        .header("x-api-key", "sk_debug")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "test-model",
            "max_tokens": 128,
            "stream": true,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    let body = response
        .text()
        .await
        .expect("stream body should be readable");
    assert!(body.contains("event: message_start"));
    assert!(body.contains("event: content_block_delta"));
    assert!(body.contains("\"text\":\"Hello\""));
    assert!(body.contains("\"text\":\" world\""));
    assert!(body.contains("event: message_stop"));
}

#[tokio::test]
#[serial]
async fn anthropic_messages_cloud_prefix_passthrough() {
    let upstream = MockServer::start().await;
    std::env::set_var("ANTHROPIC_API_KEY", "anthropic-test-key");
    std::env::set_var("ANTHROPIC_API_BASE_URL", upstream.uri());

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "anthropic-test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-7-sonnet",
            "content": [{"type": "text", "text": "Hello from Anthropic Cloud"}],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {"input_tokens": 12, "output_tokens": 7}
        })))
        .mount(&upstream)
        .await;

    let lb = spawn_test_lb().await;
    let response = Client::new()
        .post(format!("http://{}/v1/messages", lb.addr()))
        .header("x-api-key", "sk_debug")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "anthropic:claude-3-7-sonnet",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .expect("request should succeed");

    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("ANTHROPIC_API_BASE_URL");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("response must be json");
    assert_eq!(body["model"], "claude-3-7-sonnet");
    assert_eq!(body["content"][0]["text"], "Hello from Anthropic Cloud");
}

#[tokio::test]
#[serial]
async fn anthropic_messages_requires_anthropic_version_header() {
    let lb = spawn_test_lb().await;

    let response = Client::new()
        .post(format!("http://{}/v1/messages", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": "test-model",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("error body must be json");
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
#[serial]
async fn anthropic_messages_invalid_api_key_uses_anthropic_error_shape() {
    let lb = spawn_test_lb().await;

    let response = Client::new()
        .post(format!("http://{}/v1/messages", lb.addr()))
        .header("x-api-key", "invalid-key")
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": "test-model",
            "max_tokens": 128,
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
    let body: Value = response.json().await.expect("error body must be json");
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "authentication_error");
}
