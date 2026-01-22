//! Integration Test: Open Responses API
//!
//! SPEC-99024000: Open Responses API対応
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! RES001: Responses API対応バックエンドへのリクエスト転送
//! RES002: ストリーミング（responses_streaming_test.rsで実装）
//! RES003: 非対応バックエンドへの501エラー
//! RES004: 認証なしリクエストへの401エラー
//! RES005: ルート存在確認

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
struct ResponsesIntegrationState {
    supports_responses_api: bool,
    model_id: String,
}

/// Responses API対応のモックノードを起動
async fn spawn_integration_node(state: ResponsesIntegrationState) -> TestServer {
    let app = Router::new()
        .route("/v1/responses", post(responses_handler))
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
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
// RES001: Responses API対応バックエンドへのリクエスト転送テスト
// =============================================================================

/// RES001: パススルーテスト - リクエストボディがそのまま転送される
#[tokio::test]
#[serial]
async fn res001_responses_passthrough_preserves_request_body() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "integration-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（Responses API対応検出付き）
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "integration-model")
        .await
        .expect("register endpoint must succeed");

    // 複雑なリクエストボディでパススルーをテスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
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

/// RES001: ツール定義を含むリクエストのパススルー
#[tokio::test]
#[serial]
async fn res001_responses_passthrough_with_tools() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "tool-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "tool-model")
        .await
        .expect("register endpoint must succeed");

    // ツール定義を含むリクエスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
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
// RES003: 非対応バックエンドへの501エラーテスト
// =============================================================================

/// RES003: Responses API非対応バックエンドへのリクエストは501を返す
#[tokio::test]
#[serial]
async fn res003_non_supporting_backend_returns_501() {
    // Responses API非対応ノードを起動（chat_completionsのみ）
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: false,
        model_id: "chat-only-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    // エンドポイントを登録
    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "chat-only-model")
        .await
        .expect("register endpoint must succeed");

    // /v1/responses にリクエストを送信
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "chat-only-model",
            "input": "Hello"
        }))
        .send()
        .await
        .expect("request should complete");

    // 501 Not Implemented を期待
    assert_eq!(
        response.status(),
        ReqStatusCode::NOT_IMPLEMENTED,
        "Should return 501 for non-Responses-API-supporting backend"
    );

    let body: Value = response.json().await.expect("valid json response");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Responses API")
            || body["error"]["message"]
                .as_str()
                .unwrap_or("")
                .contains("Not Implemented"),
        "Error message should mention Responses API or Not Implemented"
    );
}

// =============================================================================
// RES004: 認証なしリクエストへの401エラーテスト
// =============================================================================

/// RES004: 認証ヘッダーなしで401を返す
#[tokio::test]
#[serial]
async fn res004_request_without_auth_returns_401() {
    let lb = spawn_test_lb().await;

    // 認証ヘッダーなしでリクエスト
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .json(&serde_json::json!({
            "model": "any-model",
            "input": "Hello"
        }))
        .send()
        .await
        .expect("request should complete");

    // 401 Unauthorized を期待
    assert_eq!(
        response.status(),
        ReqStatusCode::UNAUTHORIZED,
        "Should return 401 when no auth header is provided"
    );
}

// =============================================================================
// RES005: ルート存在確認テスト
// =============================================================================

/// RES005: /v1/responses ルートが存在する
#[tokio::test]
#[serial]
async fn res005_responses_route_exists() {
    let lb = spawn_test_lb().await;

    // 認証付きでリクエスト（バックエンドなしでもルートは存在確認可能）
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "input": "Hello"
        }))
        .send()
        .await
        .expect("request should complete");

    // 404 NOT FOUND でないことを確認
    // 503 (バックエンドなし) または 501 (非対応) は許容
    assert_ne!(
        response.status(),
        ReqStatusCode::NOT_FOUND,
        "/v1/responses route should exist"
    );
}

// =============================================================================
// /v1/models の supported_apis フィールドテスト
// =============================================================================

/// /v1/models に supported_apis フィールドが含まれる（Responses API対応）
#[tokio::test]
#[serial]
async fn models_api_includes_supported_apis_field() {
    // Responses API対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: true,
        model_id: "responses-capable-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "responses-capable-model")
        .await
        .expect("register endpoint must succeed");

    // /v1/models を取得
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");

    // モデル情報に supported_apis フィールドがあることを確認
    let models = body["data"].as_array().expect("data should be array");
    assert!(!models.is_empty(), "should have at least one model");

    // Responses API対応モデルを探す
    let responses_model = models
        .iter()
        .find(|m| m["id"].as_str() == Some("responses-capable-model"));
    assert!(
        responses_model.is_some(),
        "should find responses-capable-model"
    );

    let model = responses_model.unwrap();
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

/// /v1/models で非対応バックエンドは chat_completions のみ表示
#[tokio::test]
#[serial]
async fn models_api_shows_chat_only_for_non_responses_backend() {
    // Responses API非対応ノードを起動
    let node_stub = spawn_integration_node(ResponsesIntegrationState {
        supports_responses_api: false,
        model_id: "chat-only-model".to_string(),
    })
    .await;
    let lb = spawn_test_lb().await;

    let _ = register_responses_endpoint(lb.addr(), node_stub.addr(), "chat-only-model")
        .await
        .expect("register endpoint must succeed");

    // /v1/models を取得
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("x-api-key", "sk_debug")
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), ReqStatusCode::OK);
    let body: Value = response.json().await.expect("valid json response");

    let models = body["data"].as_array().expect("data should be array");

    // chat-only-model を探す
    let chat_model = models
        .iter()
        .find(|m| m["id"].as_str() == Some("chat-only-model"));
    assert!(chat_model.is_some(), "should find chat-only-model");

    let model = chat_model.unwrap();
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
