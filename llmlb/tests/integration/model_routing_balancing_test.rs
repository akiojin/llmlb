//! Integration tests for model visibility and per-model load balancing.

use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::spawn_test_lb,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde_json::{json, Value};
use serial_test::serial;

#[derive(Clone)]
struct EndpointStubState {
    endpoint_label: String,
    models: Vec<String>,
}

async fn spawn_endpoint_stub(state: EndpointStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/models", get(models_handler))
        .route("/v1/chat/completions", post(chat_handler))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn models_handler(State(state): State<Arc<EndpointStubState>>) -> impl IntoResponse {
    let data: Vec<Value> = state
        .models
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "object": "model",
                "created": 0,
                "owned_by": state.endpoint_label,
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(json!({ "object": "list", "data": data })),
    )
}

async fn chat_handler(
    State(state): State<Arc<EndpointStubState>>,
    Json(payload): Json<Value>,
) -> Response {
    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if !state.models.iter().any(|m| m == model) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
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
        Json(json!({
            "id": format!("chatcmpl-{}", state.endpoint_label),
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": format!("served-by={}", state.endpoint_label)
                },
                "finish_reason": "stop"
            }]
        })),
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
        .json(&json!({
            "name": name,
            "base_url": base_url,
            "health_check_interval_secs": 30
        }))
        .send()
        .await
        .expect("create endpoint request");
    assert_eq!(create_resp.status(), reqwest::StatusCode::CREATED);

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
    assert_eq!(test_resp.status(), reqwest::StatusCode::OK);

    let sync_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint sync request");
    assert_eq!(sync_resp.status(), reqwest::StatusCode::OK);

    endpoint_id
}

#[tokio::test]
#[serial]
async fn v1_models_returns_union_across_endpoints() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    let ep1 = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-1".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep2 = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-2".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep3 = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-3".to_string(),
        models: vec!["shared-model".to_string(), "another-model".to_string()],
    })
    .await;

    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Union Endpoint 1",
        &format!("http://{}", ep1.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Union Endpoint 2",
        &format!("http://{}", ep2.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Union Endpoint 3",
        &format!("http://{}", ep3.addr()),
    )
    .await;

    let models_resp = client
        .get(format!("http://{}/v1/models", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("v1 models request");
    assert_eq!(models_resp.status(), reqwest::StatusCode::OK);

    let body: Value = models_resp.json().await.expect("v1 models json");
    let data = body["data"].as_array().expect("data array");

    let ids: HashSet<String> = data
        .iter()
        .filter_map(|m| m["id"].as_str().map(ToOwned::to_owned))
        .collect();

    assert!(ids.contains("shared-model"));
    assert!(
        ids.contains("qwen3-coder:30b"),
        "model existing on subset of endpoints must be listed"
    );
    assert!(ids.contains("another-model"));

    let qwen_count = data
        .iter()
        .filter(|m| m["id"].as_str() == Some("qwen3-coder:30b"))
        .count();
    assert_eq!(
        qwen_count, 1,
        "model should not be duplicated in /v1/models"
    );
}

#[tokio::test]
#[serial]
async fn chat_completions_balances_across_endpoints_having_same_model() {
    let lb = spawn_test_lb().await;
    let client = Client::new();

    let ep_a = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-a".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep_b = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-b".to_string(),
        models: vec!["shared-model".to_string(), "qwen3-coder:30b".to_string()],
    })
    .await;
    let ep_c = spawn_endpoint_stub(EndpointStubState {
        endpoint_label: "ep-c".to_string(),
        models: vec!["shared-model".to_string()],
    })
    .await;

    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Balance Endpoint A",
        &format!("http://{}", ep_a.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Balance Endpoint B",
        &format!("http://{}", ep_b.addr()),
    )
    .await;
    register_and_sync_endpoint(
        &client,
        lb.addr(),
        "Balance Endpoint C",
        &format!("http://{}", ep_c.addr()),
    )
    .await;

    let mut served_by: HashSet<String> = HashSet::new();

    for _ in 0..12 {
        let resp = client
            .post(format!("http://{}/v1/chat/completions", lb.addr()))
            .header("x-api-key", "sk_debug")
            .json(&json!({
                "model": "qwen3-coder:30b",
                "messages": [{"role": "user", "content": "ping"}],
                "stream": false
            }))
            .send()
            .await
            .expect("chat request");

        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: Value = resp.json().await.expect("chat response json");
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .expect("assistant content")
            .to_string();
        served_by.insert(content);
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
