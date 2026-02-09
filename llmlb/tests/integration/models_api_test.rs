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
}

/// Responses API対応のモックノードを起動
async fn spawn_model_node(state: ModelNodeState) -> TestServer {
    let app = Router::new()
        .route("/v1/models", get(models_handler))
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
/// - 常に ["chat_completions", "responses"] を含むこと
#[tokio::test]
#[serial]
async fn v1_models_includes_supported_apis_field() {
    let node_state = ModelNodeState {
        model_id: "test-model-with-responses".to_string(),
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

// ===== SPEC-6cd7f960 4.7: エンドポイント集約動作テスト =====

/// 4.7: エンドポイント集約 - 複数エンドポイントのモデルが/v1/modelsに集約される
#[tokio::test]
#[serial]
async fn v1_models_aggregates_multiple_endpoints() {
    // 2つの異なるモデルを持つエンドポイントを起動
    let node1_state = ModelNodeState {
        model_id: "model-from-endpoint-1".to_string(),
    };
    let node2_state = ModelNodeState {
        model_id: "model-from-endpoint-2".to_string(),
    };
    let stub1 = spawn_model_node(node1_state).await;
    let stub2 = spawn_model_node(node2_state).await;

    // ルーターを起動
    let lb = spawn_test_lb().await;

    // 両エンドポイントを登録
    let _ = register_responses_endpoint(lb.addr(), stub1.addr(), "model-from-endpoint-1")
        .await
        .expect("register endpoint 1");
    let _ = register_responses_endpoint(lb.addr(), stub2.addr(), "model-from-endpoint-2")
        .await
        .expect("register endpoint 2");

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

    // 両エンドポイントのモデルが含まれていることを確認
    let model1 = data
        .iter()
        .find(|m| m["id"].as_str() == Some("model-from-endpoint-1"));
    let model2 = data
        .iter()
        .find(|m| m["id"].as_str() == Some("model-from-endpoint-2"));

    assert!(
        model1.is_some(),
        "model from endpoint 1 should be aggregated"
    );
    assert!(
        model2.is_some(),
        "model from endpoint 2 should be aggregated"
    );
}

/// 4.7: エンドポイント集約 - オンラインエンドポイントにないモデルは/v1/modelsに含まれない
/// SPEC-6cd7f960 FR-6: 利用可能なモデルのみを返す
#[tokio::test]
#[serial]
async fn v1_models_excludes_models_not_on_endpoints() {
    // ルーターを起動
    let lb = spawn_test_lb().await;

    // モデルを登録（ただしエンドポイントは登録しない）
    let client = Client::new();
    let register_resp = client
        .post(format!("http://{}/api/models/register", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "repo": "test/orphan-model"
        }))
        .send()
        .await
        .expect("register model request");

    // 登録は成功するはず（201 Created）
    assert!(
        register_resp.status() == reqwest::StatusCode::CREATED
            || register_resp.status() == reqwest::StatusCode::BAD_REQUEST,
        "register should succeed or fail with validation error"
    );

    // /v1/models を呼び出し
    let response = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list models request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let body: Value = response.json().await.expect("parse json");
    let data = body["data"].as_array().expect("data array");

    // エンドポイントにないモデルは含まれないことを確認
    // （オフラインのため、登録済みでもリストに含まれない）
    let orphan_model = data.iter().find(|m| {
        m["id"]
            .as_str()
            .map(|s| s.contains("orphan"))
            .unwrap_or(false)
    });

    assert!(
        orphan_model.is_none(),
        "registered model without online endpoint should NOT be in /v1/models"
    );
}
