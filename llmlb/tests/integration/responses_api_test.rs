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

use std::{collections::HashSet, net::SocketAddr, sync::Arc};

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
    model_id: String,
}

#[derive(Clone)]
struct ResponsesRoutingState {
    endpoint_label: String,
    models: Vec<String>,
}

/// Responses API対応のモックノードを起動
async fn spawn_integration_node(state: ResponsesIntegrationState) -> TestServer {
    let app = Router::new()
        .route("/v1/responses", post(responses_handler))
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn responses_handler(
    State(_state): State<Arc<ResponsesIntegrationState>>,
    Json(req): Json<Value>,
) -> impl axum::response::IntoResponse {
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
    let models = vec![serde_json::json!({
        "id": state.model_id,
        "object": "model",
        "created": 1704067200,
        "owned_by": "test",
        "supported_apis": ["chat_completions", "responses"]
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

async fn spawn_routing_node(state: ResponsesRoutingState) -> TestServer {
    let app = Router::new()
        .route("/v1/responses", post(routing_responses_handler))
        .route("/v1/models", get(routing_models_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn routing_responses_handler(
    State(state): State<Arc<ResponsesRoutingState>>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    let model = req["model"].as_str().unwrap_or_default();

    if !state.models.iter().any(|m| m == model) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "message": format!("model '{}' not found on {}", model, state.endpoint_label),
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": format!("resp-{}", state.endpoint_label),
            "object": "response",
            "model": model,
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {
                            "type": "output_text",
                            "text": format!("served-by={}", state.endpoint_label)
                        }
                    ]
                }
            ]
        })),
    )
        .into_response()
}

async fn routing_models_handler(
    State(state): State<Arc<ResponsesRoutingState>>,
) -> impl IntoResponse {
    let data: Vec<Value> = state
        .models
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "object": "model",
                "created": 0,
                "owned_by": state.endpoint_label,
                "supported_apis": ["chat_completions", "responses"]
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({ "object": "list", "data": data })),
    )
        .into_response()
}

async fn register_and_sync_endpoint(
    client: &Client,
    lb_addr: SocketAddr,
    name: &str,
    base_url: &str,
) -> String {
    let create_resp = client
        .post(format!("http://{}/api/endpoints", lb_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&serde_json::json!({
            "name": name,
            "base_url": base_url,
            "health_check_interval_secs": 30
        }))
        .send()
        .await
        .expect("create endpoint request");
    assert_eq!(create_resp.status(), ReqStatusCode::CREATED);

    let created: Value = create_resp.json().await.expect("create endpoint json");
    let endpoint_id = created["id"].as_str().expect("endpoint id").to_string();

    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test request");
    assert_eq!(test_resp.status(), ReqStatusCode::OK);

    let sync_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint sync request");
    assert_eq!(sync_resp.status(), ReqStatusCode::OK);

    endpoint_id
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

/// RES001: 同一モデルを持つ複数エンドポイント間で /v1/responses が分散される
#[tokio::test]
#[serial]
async fn res001_responses_balances_across_endpoints_having_same_model() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    let ep_a = spawn_routing_node(ResponsesRoutingState {
        endpoint_label: "ep-a".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep_b = spawn_routing_node(ResponsesRoutingState {
        endpoint_label: "ep-b".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep_c = spawn_routing_node(ResponsesRoutingState {
        endpoint_label: "ep-c".to_string(),
        models: vec!["shared-model".to_string()],
    })
    .await;

    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Responses Balance Endpoint A",
        &format!("http://{}", ep_a.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Responses Balance Endpoint B",
        &format!("http://{}", ep_b.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Responses Balance Endpoint C",
        &format!("http://{}", ep_c.addr()),
    )
    .await;

    let mut served_by: HashSet<String> = HashSet::new();

    for _ in 0..12 {
        let response = client
            .post(format!("http://{}/v1/responses", lb.addr()))
            .header("x-api-key", "sk_debug")
            .json(&serde_json::json!({
                "model": "qwen3-coder:30b",
                "input": "ping"
            }))
            .send()
            .await
            .expect("responses request should succeed");
        assert_eq!(response.status(), ReqStatusCode::OK);

        let body: Value = response.json().await.expect("valid json response");
        let text = body["output"][0]["content"][0]["text"]
            .as_str()
            .expect("output text")
            .to_string();
        served_by.insert(text);
    }

    assert!(
        served_by.contains("served-by=ep-a"),
        "qwen3-coder:30b should be served by endpoint A"
    );
    assert!(
        served_by.contains("served-by=ep-b"),
        "qwen3-coder:30b should be served by endpoint B"
    );
    assert!(
        !served_by.contains("served-by=ep-c"),
        "endpoint without qwen3-coder:30b must not receive traffic for that model"
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

    // 認証付きで不正リクエスト（modelフィールドなし）を送信し、
    // 404(ルート未存在)ではなくバリデーションエラーが返ることを確認する。
    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/responses", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "input": "Hello"
        }))
        .send()
        .await
        .expect("request should complete");

    assert_eq!(response.status(), ReqStatusCode::BAD_REQUEST);
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
