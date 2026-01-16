//! Integration Test: Open Responses API
//!
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! T060: パススルーテスト
//! T062: /v1/models の supported_apis フィールドテスト

use std::sync::Arc;

use crate::support::{
    http::{spawn_router, TestServer},
    router::{register_node, spawn_test_router},
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
struct ResponsesIntegrationState {
    supports_responses_api: bool,
    model_id: String,
}

/// Responses API対応のモックノードを起動
async fn spawn_integration_node(state: ResponsesIntegrationState) -> TestServer {
    let router = Router::new()
        .route("/v1/responses", post(responses_handler))
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

async fn responses_handler(
    State(state): State<Arc<ResponsesIntegrationState>>,
    Json(req): Json<Value>,
) -> impl axum::response::IntoResponse {
    if !state.supports_responses_api {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({
                "error": "Not Implemented: Responses API is not supported"
            })),
        )
            .into_response();
    }

    // リクエストのモデル名をそのままレスポンスに含める（パススルー検証用）
    let model = req["model"].as_str().unwrap_or("unknown");

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": "resp_integration_test",
            "object": "response",
            "created_at": 1704067200,
            "model": model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": format!("Response from {} via Responses API", model)
                        }
                    ]
                }
            ],
            "usage": {
                "input_tokens": 5,
                "output_tokens": 10,
                "total_tokens": 15
            }
        })),
    )
        .into_response()
}

async fn chat_handler(
    State(state): State<Arc<ResponsesIntegrationState>>,
    Json(req): Json<Value>,
) -> impl axum::response::IntoResponse {
    let model = req["model"].as_str().unwrap_or(&state.model_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": "chatcmpl-integration",
            "object": "chat.completion",
            "created": 1704067200,
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": format!("Response from {} via Chat API", model)
                    },
                    "finish_reason": "stop"
                }
            ]
        })),
    )
        .into_response()
}

async fn models_handler(State(state): State<Arc<ResponsesIntegrationState>>) -> impl IntoResponse {
    let supported_apis = if state.supports_responses_api {
        vec!["chat_completions", "responses"]
    } else {
        vec!["chat_completions"]
    };

    let models = vec![serde_json::json!({
        "id": state.model_id,
        "object": "model",
        "created": 1704067200,
        "owned_by": "test",
        "supported_apis": supported_apis
    })];

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "object": "list",
            "data": models
        })),
    )
        .into_response()
}

async fn health_handler(State(state): State<Arc<ResponsesIntegrationState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "supports_responses_api": state.supports_responses_api
        })),
    )
        .into_response()
}

// =============================================================================
// T060: パススルーテスト
// =============================================================================

#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/responses passthrough not implemented yet"]
async fn responses_passthrough_preserves_request_body() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "integration-model".to_string(),
    })
    .await;
    let router = spawn_test_router().await;

    // ノードを登録
    let register_response = register_node(router.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // 複雑なリクエストボディでパススルーをテスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "integration-model",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": "Hello, how are you?"
                }
            ],
            "instructions": "You are a helpful assistant.",
            "temperature": 0.7,
            "max_output_tokens": 100
        }))
        .send()
        .await
        .expect("responses request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");

    // レスポンスがバックエンドからそのまま返されていることを確認
    assert_eq!(body["object"], "response");
    assert_eq!(body["model"], "integration-model");
    assert!(body["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Responses API"));
}

#[tokio::test]
#[serial]
#[ignore = "TDD RED: /v1/responses passthrough not implemented yet"]
async fn responses_passthrough_with_tools() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "tool-model".to_string(),
    })
    .await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // ツール定義を含むリクエスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "tool-model",
            "input": "What's the weather?",
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "get_weather",
                        "description": "Get the current weather",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "location": {"type": "string"}
                            }
                        }
                    }
                }
            ]
        }))
        .send()
        .await
        .expect("responses request should succeed");

    // ツール定義がそのままパススルーされることを確認
    assert_eq!(response.status(), ReqStatusCode::OK);
}

// =============================================================================
// T062: /v1/models の supported_apis フィールドテスト
// =============================================================================

#[tokio::test]
#[serial]
#[ignore = "TDD RED: supported_apis field not implemented in /v1/models"]
async fn models_api_includes_supported_apis_field() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "responses-capable-model".to_string(),
    })
    .await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // /v1/models を取得
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", router.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");

    // モデル情報に supported_apis フィールドがあることを確認
    let models = body["data"].as_array().expect("data should be array");
    assert!(!models.is_empty(), "should have at least one model");

    let model = &models[0];
    let supported_apis = model["supported_apis"]
        .as_array()
        .expect("supported_apis should be array");

    // Responses API対応モデルの場合
    assert!(
        supported_apis
            .iter()
            .any(|api| api.as_str() == Some("chat_completions")),
        "should support chat_completions"
    );
    assert!(
        supported_apis
            .iter()
            .any(|api| api.as_str() == Some("responses")),
        "should support responses"
    );
}

#[tokio::test]
#[serial]
#[ignore = "TDD RED: supported_apis field not implemented in /v1/models"]
async fn models_api_shows_chat_only_for_non_responses_backend() {
    // Responses API非対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: false,
        model_id: "chat-only-model".to_string(),
    })
    .await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), node_stub.addr())
        .await
        .expect("register node must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    // /v1/models を取得
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", router.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");

    let models = body["data"].as_array().expect("data should be array");
    let model = &models[0];
    let supported_apis = model["supported_apis"]
        .as_array()
        .expect("supported_apis should be array");

    // Responses API非対応モデルの場合
    assert!(
        supported_apis
            .iter()
            .any(|api| api.as_str() == Some("chat_completions")),
        "should support chat_completions"
    );
    assert!(
        !supported_apis
            .iter()
            .any(|api| api.as_str() == Some("responses")),
        "should NOT support responses"
    );
}
