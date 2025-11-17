//! Contract Test: OpenAI /v1/completions proxy

use std::sync::Arc;

use crate::support::{
    coordinator::{register_agent, spawn_coordinator},
    http::{spawn_router, TestServer},
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use ollama_coordinator_common::protocol::GenerateRequest;
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::Value;

#[derive(Clone)]
struct AgentStubState {
    expected_model: Option<String>,
    response: AgentGenerateStubResponse,
}

#[derive(Clone)]
enum AgentGenerateStubResponse {
    Success(Value),
    Error(StatusCode, String),
}

async fn spawn_agent_stub(state: AgentStubState) -> TestServer {
    let router = Router::new()
        .route("/api/generate", post(agent_generate_handler))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

async fn agent_generate_handler(
    State(state): State<Arc<AgentStubState>>,
    Json(req): Json<GenerateRequest>,
) -> impl axum::response::IntoResponse {
    if let Some(expected) = &state.expected_model {
        assert_eq!(
            &req.model, expected,
            "coordinator should proxy the requested model name"
        );
    }

    match &state.response {
        AgentGenerateStubResponse::Success(payload) => {
            (StatusCode::OK, Json(payload.clone())).into_response()
        }
        AgentGenerateStubResponse::Error(status, body) => (*status, body.clone()).into_response(),
    }
}

#[tokio::test]
async fn proxy_completions_end_to_end_success() {
    let agent_stub = spawn_agent_stub(AgentStubState {
        expected_model: Some("gpt-oss:20b".to_string()),
        response: AgentGenerateStubResponse::Success(serde_json::json!({
            "id": "cmpl-123",
            "object": "text_completion",
            "choices": [
                {"text": "hello from stub", "index": 0, "logprobs": null, "finish_reason": "stop"}
            ]
        })),
    })
    .await;
    let coordinator = spawn_coordinator().await;

    let register_response = register_agent(coordinator.addr(), agent_stub.addr())
        .await
        .expect("register agent must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::OK);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/completions", coordinator.addr()))
        .json(&serde_json::json!({
            "model": "gpt-oss:20b",
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
async fn proxy_completions_propagates_upstream_error() {
    let agent_stub = spawn_agent_stub(AgentStubState {
        expected_model: Some("missing-model".to_string()),
        response: AgentGenerateStubResponse::Error(
            StatusCode::BAD_REQUEST,
            "model not loaded".to_string(),
        ),
    })
    .await;
    let coordinator = spawn_coordinator().await;

    let register_response = register_agent(coordinator.addr(), agent_stub.addr())
        .await
        .expect("register agent must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::OK);

    let client = Client::new();
    let response = client
        .post(format!("http://{}/v1/completions", coordinator.addr()))
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
