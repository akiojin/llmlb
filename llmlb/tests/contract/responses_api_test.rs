//! Contract Test: Open Responses API (/v1/responses)
//!
//! SPEC-0f1de549: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! これらのテストはTDD REDフェーズとして作成され、実装前に失敗する。

use std::sync::Arc;

use crate::support::{
    http::{spawn_lb, TestServer},
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
use serde_json::Value;
use serial_test::serial;

#[derive(Clone)]
struct ResponsesNodeStubState {
    response: ResponsesStubResponse,
}

#[derive(Clone)]
enum ResponsesStubResponse {
    Success(Value),
    Error(StatusCode, String),
}

/// Responses API対応のモックノードを起動
async fn spawn_responses_node_stub(state: ResponsesNodeStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/responses", post(responses_handler))
        .route("/v1/models", get(models_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn responses_handler(
    State(state): State<Arc<ResponsesNodeStubState>>,
    Json(_req): Json<Value>,
) -> impl axum::response::IntoResponse {
    match &state.response {
        ResponsesStubResponse::Success(payload) => {
            (StatusCode::OK, Json(payload.clone())).into_response()
        }
        ResponsesStubResponse::Error(status, body) => (*status, body.clone()).into_response(),
    }
}

async fn models_handler(State(_state): State<Arc<ResponsesNodeStubState>>) -> impl IntoResponse {
    let models = vec![serde_json::json!({
        "id": "test-model",
        "object": "model"
    })];

    (StatusCode::OK, Json(serde_json::json!({"data": models}))).into_response()
}

// =============================================================================
// T057: POST /v1/responses 基本リクエストテスト
// =============================================================================

#[tokio::test]
#[serial]
async fn responses_api_basic_request_success() {
    // Responses API対応ノードをスタブとして起動
    let node_stub = spawn_responses_node_stub(ResponsesNodeStubState {
        response: ResponsesStubResponse::Success(serde_json::json!({
            "id": "resp_123",
            "object": "response",
            "created_at": 1704067200,
            "model": "test-model",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello! How can I help you?"
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 8,
                "total_tokens": 18
            }
        })),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "test-model")
        .await
        .expect("register endpoint must succeed");

    // /v1/responses にリクエストを送信
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello!"
        }))
        .send()
        .await
        .expect("responses request should succeed");

    // パススルーされたレスポンスを確認
    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");
    assert_eq!(body["object"], "response");
    assert_eq!(
        body["output"][0]["content"][0]["text"],
        "Hello! How can I help you?"
    );
}

// =============================================================================
// T058: バックエンドのエラーステータスをパススルーする
// =============================================================================

#[tokio::test]
#[serial]
async fn responses_api_passthroughs_backend_error_status() {
    let node_stub = spawn_responses_node_stub(ResponsesNodeStubState {
        response: ResponsesStubResponse::Error(
            StatusCode::NOT_IMPLEMENTED,
            "backend_not_implemented".to_string(),
        ),
    })
    .await;
    let lb = spawn_test_lb().await;

    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "test-model")
        .await
        .expect("register endpoint must succeed");

    // /v1/responses にリクエストを送信
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello!"
        }))
        .send()
        .await
        .expect("responses request should complete");

    assert_eq!(response.status(), ReqStatusCode::NOT_IMPLEMENTED);
    let body = response.text().await.expect("read body text");
    assert_eq!(body, "backend_not_implemented");
}

// =============================================================================
// T059: 認証必須テスト
// =============================================================================

#[tokio::test]
#[serial]
async fn responses_api_requires_authentication() {
    let lb = spawn_test_lb().await;

    // 認証ヘッダーなしでリクエスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello!"
        }))
        .send()
        .await
        .expect("responses request should complete");

    // 401 Unauthorizedを確認
    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn responses_api_rejects_invalid_api_key() {
    let lb = spawn_test_lb().await;

    // 無効なAPIキーでリクエスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "invalid_key")
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello!"
        }))
        .send()
        .await
        .expect("responses request should complete");

    // 401 Unauthorizedを確認
    assert_eq!(response.status(), ReqStatusCode::UNAUTHORIZED);
}

// =============================================================================
// ストリーミングテスト（補足）
// =============================================================================

#[tokio::test]
#[serial]
async fn responses_api_streaming_passthrough() {
    // ストリーミングレスポンスのパススルーテストは
    // integration testで詳細に行う（T061）
    // ここでは基本的なstream=trueのリクエスト受付を確認

    let node_stub = spawn_responses_node_stub(ResponsesNodeStubState {
        response: ResponsesStubResponse::Success(serde_json::json!({
            "id": "resp_123",
            "object": "response"
        })),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "test-model")
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello!",
            "stream": true
        }))
        .send()
        .await
        .expect("streaming responses request should succeed");

    // ストリーミングリクエストが受け付けられることを確認
    assert_eq!(response.status(), ReqStatusCode::OK);
}
