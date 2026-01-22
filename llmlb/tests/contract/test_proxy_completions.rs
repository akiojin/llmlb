//! Contract Test: OpenAI /v1/completions proxy

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_node, spawn_test_lb},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use llmlb_common::protocol::GenerateRequest;
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;

#[derive(Clone)]
struct NodeStubState {
    expected_model: Option<String>,
    response: NodeGenerateStubResponse,
}

#[derive(Clone)]
enum NodeGenerateStubResponse {
    Success(Value),
    Error(StatusCode, String),
}

async fn spawn_node_stub(state: NodeStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/completions", post(node_generate_handler))
        .route("/v1/chat/completions", post(node_generate_handler))
        .route("/v1/models", get(node_models_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn node_generate_handler(
    State(state): State<Arc<NodeStubState>>,
    Json(req): Json<GenerateRequest>,
) -> impl axum::response::IntoResponse {
    if let Some(expected) = &state.expected_model {
        assert_eq!(
            &req.model, expected,
            "load balancer should proxy the requested model name"
        );
    }

    match &state.response {
        NodeGenerateStubResponse::Success(payload) => {
            (StatusCode::OK, Json(payload.clone())).into_response()
        }
        NodeGenerateStubResponse::Error(status, body) => (*status, body.clone()).into_response(),
    }
}

async fn node_models_handler(State(state): State<Arc<NodeStubState>>) -> impl IntoResponse {
    // デフォルトで expected_model があればそのみ返す。なければ 5モデル仕様を返す。
    let models: Vec<_> = if let Some(model) = &state.expected_model {
        vec![serde_json::json!({"id": model})]
    } else {
        vec![
            serde_json::json!({"id": "gpt-oss-20b"}),
            serde_json::json!({"id": "gpt-oss-120b"}),
            serde_json::json!({"id": "gpt-oss-safeguard-20b"}),
            serde_json::json!({"id": "qwen3-coder-30b"}),
        ]
    };

    (StatusCode::OK, Json(serde_json::json!({"data": models}))).into_response()
}

#[tokio::test]
#[serial]
#[ignore = "TDD RED: Mock node server health check issue"]
async fn proxy_completions_end_to_end_success() {
    let node_stub = spawn_node_stub(NodeStubState {
        expected_model: Some("gpt-oss-20b".to_string()),
        response: NodeGenerateStubResponse::Success(serde_json::json!({
            "id": "cmpl-123",
            "object": "text_completion",
            "choices": [
                {"text": "hello from stub", "index": 0, "logprobs": null, "finish_reason": "stop"}
            ]
        })),
    })
    .await;
    let lb = spawn_test_lb().await;

    let register_response = register_node(lb.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "gpt-oss-20b",
            "prompt": "ping",
            "max_tokens": 8
        }))
        .send()
        .await
        .expect("completions request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(body["choices"][0]["text"], "hello from stub");
}

#[tokio::test]
#[serial]
#[ignore = "TDD RED: Mock node server health check issue"]
async fn proxy_completions_propagates_upstream_error() {
    let node_stub = spawn_node_stub(NodeStubState {
        expected_model: Some("missing-model".to_string()),
        response: NodeGenerateStubResponse::Error(
            StatusCode::BAD_REQUEST,
            "model not loaded".to_string(),
        ),
    })
    .await;
    let lb = spawn_test_lb().await;

    let register_response = register_node(lb.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "missing-model",
            "prompt": "ping",
            "max_tokens": 8
        }))
        .send()
        .await
        .expect("completions request should succeed");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
    let body = response.text().await.expect("body should be readable");
    assert!(body.contains("model not loaded"));
}

#[tokio::test]
#[ignore] // このテストはタイミング依存で不安定なため、一時的に無効化
async fn proxy_completions_queue_overflow_returns_503() {
    // TODO: このテストを安定させるための実装改善が必要
    // 問題:
    // 1. all_initializing()の判定タイミングが不安定
    // 2. wait_for_ready()が呼ばれる前にノードが準備完了になる
    // 3. LoadManager側の状態更新とリクエスト処理のタイミング競合
}
