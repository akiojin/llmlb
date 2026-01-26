//! Integration Test: /v1/models API with supported_apis field
//!
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! T062: supported_apis フィールドテスト

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
use reqwest::Client;
use serde_json::{json, Value};
use serial_test::serial;

#[derive(Clone)]
struct ModelNodeState {
    model_id: String,
    supports_responses_api: bool,
}

/// Responses API対応のモックノードを起動
async fn spawn_model_node(state: ModelNodeState) -> TestServer {
    let app = Router::new()
        .route("/v1/models", get(models_handler))
        .route("/health", get(health_handler))
        .route("/v1/responses", post(responses_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn models_handler(State(state): State<Arc<ModelNodeState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "data": [
                {
                    "id": state.model_id,
                    "object": "model",
                    "created": 0,
                    "owned_by": "test"
                }
            ]
        })),
    )
}

async fn health_handler(State(state): State<Arc<ModelNodeState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "supports_responses_api": state.supports_responses_api
        })),
    )
}

async fn responses_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "id": "resp_test",
            "output": [{"type": "message", "content": [{"type": "output_text", "text": "ok"}]}]
        })),
    )
}

/// T062: /v1/models レスポンスに supported_apis フィールドが含まれることを確認
/// - Responses API対応エンドポイントの場合: ["chat_completions", "responses"]
/// - 非対応の場合: ["chat_completions"]
#[tokio::test]
#[serial]
async fn v1_models_includes_supported_apis_field() {
    // Responses API対応のモックノードを起動
    let node_state = ModelNodeState {
        model_id: "test-model-with-responses".to_string(),
        supports_responses_api: true,
    };
    let stub = spawn_model_node(node_state).await;

    // llmlbを起動
    let lb = spawn_test_lb().await;

    // エンドポイントを登録（支援関数を使用）
    let _endpoint_id =
        register_responses_endpoint(lb.addr(), stub.addr(), "test-model-with-responses")
            .await
            .expect("register endpoint");

    // /v1/models を呼び出し
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list models request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let body: Value = response.json().await.expect("parse json");
    let data = body["data"].as_array().expect("data array");

    // 登録したモデルを検索
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("test-model-with-responses"));

    assert!(model.is_some(), "model should be in /v1/models response");
    let model = model.unwrap();

    // supported_apis フィールドが存在することを確認
    assert!(
        model.get("supported_apis").is_some(),
        "supported_apis field should exist in model response"
    );

    let supported_apis = model["supported_apis"]
        .as_array()
        .expect("supported_apis should be array");

    // Responses API対応エンドポイントの場合は responses が含まれる
    let api_strings: Vec<&str> = supported_apis.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        api_strings.contains(&"chat_completions"),
        "should contain chat_completions"
    );
    assert!(
        api_strings.contains(&"responses"),
        "should contain responses for Responses API capable endpoint"
    );
}

/// Responses API非対応エンドポイントの場合は responses が含まれない
#[tokio::test]
#[serial]
async fn v1_models_excludes_responses_api_for_non_supporting_endpoint() {
    // Responses API非対応のモックノードを起動
    let node_state = ModelNodeState {
        model_id: "test-model-no-responses".to_string(),
        supports_responses_api: false,
    };
    let stub = spawn_model_node(node_state).await;

    // llmlbを起動
    let lb = spawn_test_lb().await;

    // エンドポイントを登録
    let _endpoint_id =
        register_responses_endpoint(lb.addr(), stub.addr(), "test-model-no-responses")
            .await
            .expect("register endpoint");

    // /v1/models を呼び出し
    let client = Client::new();
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list models request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let body: Value = response.json().await.expect("parse json");
    let data = body["data"].as_array().expect("data array");

    // 登録したモデルを検索
    let model = data
        .iter()
        .find(|m| m["id"].as_str() == Some("test-model-no-responses"));

    assert!(model.is_some(), "model should be in /v1/models response");
    let model = model.unwrap();

    // supported_apis フィールドを確認
    let supported_apis = model["supported_apis"]
        .as_array()
        .expect("supported_apis should be array");

    let api_strings: Vec<&str> = supported_apis.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        api_strings.contains(&"chat_completions"),
        "should contain chat_completions"
    );
    assert!(
        !api_strings.contains(&"responses"),
        "should NOT contain responses for non-supporting endpoint"
    );
}
