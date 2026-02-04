//! Contract Test: OpenAI /v1/completions proxy

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{spawn_test_lb, spawn_test_lb_with_manager},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use llmlb::{
    balancer::{LoadManager, MetricsUpdate},
    common::protocol::GenerateRequest,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::Value;
use serial_test::serial;
use tokio::sync::Notify;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

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

#[derive(Clone)]
struct QueueStubState {
    request_count: Arc<AtomicUsize>,
    first_started: Arc<Notify>,
    release_first: Arc<Notify>,
}

impl QueueStubState {
    fn new() -> Self {
        Self {
            request_count: Arc::new(AtomicUsize::new(0)),
            first_started: Arc::new(Notify::new()),
            release_first: Arc::new(Notify::new()),
        }
    }
}

async fn spawn_queue_stub(state: QueueStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(queue_chat_handler))
        .route("/v1/models", get(queue_models_handler))
        .route("/health", get(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn queue_chat_handler(
    State(state): State<Arc<QueueStubState>>,
    Json(_): Json<serde_json::Value>,
) -> impl IntoResponse {
    let count = state.request_count.fetch_add(1, Ordering::SeqCst);
    if count == 0 {
        state.first_started.notify_waiters();
        state.release_first.notified().await;
    }

    let body = serde_json::json!({
        "id": "queue-stub",
        "object": "chat.completion",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
    });

    (StatusCode::OK, Json(body)).into_response()
}

async fn queue_models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({"data": [{"id": "test-model", "object": "model"}]})),
    )
        .into_response()
}

fn set_queue_env(max: usize, timeout_secs: u64) {
    std::env::set_var("LLMLB_QUEUE_MAX", max.to_string());
    std::env::set_var("LLMLB_QUEUE_TIMEOUT_SECS", timeout_secs.to_string());
}

fn clear_queue_env() {
    std::env::remove_var("LLMLB_QUEUE_MAX");
    std::env::remove_var("LLMLB_QUEUE_TIMEOUT_SECS");
}

#[tokio::test]
#[serial]
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
    let client = Client::new();

    // Endpoint登録 + モデル同期（Node登録APIは廃止済み）
    let register_response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&serde_json::json!({
            "name": "stub-endpoint",
            "base_url": format!("http://{}", node_stub.addr())
        }))
        .send()
        .await
        .expect("endpoint registration must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let register_body: Value = register_response
        .json()
        .await
        .expect("endpoint registration response must be json");
    let endpoint_id = register_body["id"]
        .as_str()
        .expect("endpoint id must be present");

    // ヘルスチェックでオンライン化（routingはonlineのみ対象）
    let test_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test must succeed");
    assert_eq!(test_response.status(), ReqStatusCode::OK);

    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("model sync must succeed");
    assert_eq!(sync_response.status(), ReqStatusCode::OK);

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
    let client = Client::new();

    // Endpoint登録 + モデル同期（Node登録APIは廃止済み）
    let register_response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&serde_json::json!({
            "name": "stub-endpoint-error",
            "base_url": format!("http://{}", node_stub.addr())
        }))
        .send()
        .await
        .expect("endpoint registration must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let register_body: Value = register_response
        .json()
        .await
        .expect("endpoint registration response must be json");
    let endpoint_id = register_body["id"]
        .as_str()
        .expect("endpoint id must be present");

    // ヘルスチェックでオンライン化（routingはonlineのみ対象）
    let test_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test must succeed");
    assert_eq!(test_response.status(), ReqStatusCode::OK);

    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("model sync must succeed");
    assert_eq!(sync_response.status(), ReqStatusCode::OK);

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

    let status = response.status();
    assert!(
        status == ReqStatusCode::BAD_REQUEST || status == ReqStatusCode::BAD_GATEWAY,
        "upstream error should be propagated as 400 or mapped to 502, got {status}"
    );
    let body = response.text().await.expect("body should be readable");
    if status == ReqStatusCode::BAD_REQUEST {
        assert!(body.contains("model not loaded"));
    } else {
        assert!(
            !body.trim().is_empty(),
            "502 responses should still include an error body"
        );
    }
}

#[tokio::test]
async fn proxy_completions_queue_overflow_returns_429() {
    set_queue_env(0, 5);

    let stub_state = QueueStubState::new();
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let (lb, load_manager) = spawn_test_lb_with_manager().await;
    let client = Client::new();

    let register_response = client
        .post(format!("http://{}/api/endpoints", lb.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&serde_json::json!({
            "name": "queue-stub",
            "base_url": format!("http://{}", stub.addr())
        }))
        .send()
        .await
        .expect("endpoint registration must succeed");
    assert_eq!(register_response.status(), ReqStatusCode::CREATED);

    let register_body: Value = register_response
        .json()
        .await
        .expect("endpoint registration response must be json");
    let endpoint_id = register_body["id"]
        .as_str()
        .expect("endpoint id must be present");

    let test_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint test must succeed");
    assert_eq!(test_response.status(), ReqStatusCode::OK);

    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("model sync must succeed");
    assert_eq!(sync_response.status(), ReqStatusCode::OK);

    let first = tokio::spawn({
        let client = client.clone();
        let addr = lb.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&serde_json::json!({
                    "model": "test-model",
                    "messages": [{"role": "user", "content": "ping"}]
                }))
                .send()
                .await
                .expect("first request should be sent")
        }
    });

    timeout(Duration::from_secs(1), stub_state.first_started.notified())
        .await
        .expect("first request should reach node");
    mark_endpoint_busy(&load_manager, endpoint_id).await;
    wait_for_endpoint_active(&load_manager, endpoint_id, Duration::from_secs(1)).await;

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&serde_json::json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "ping"}]
        }))
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::TOO_MANY_REQUESTS);
    assert!(second_resp.headers().get("retry-after").is_some());
    let body: Value = second_resp.json().await.expect("second response json");
    assert_eq!(
        body["error"]["message"],
        serde_json::Value::String("Request queue is full".to_string())
    );

    stub_state.release_first.notify_waiters();

    let first_resp = first.await.expect("first join should succeed");
    assert_eq!(first_resp.status(), ReqStatusCode::OK);

    clear_queue_env();
}

async fn wait_for_endpoint_active(
    load_manager: &LoadManager,
    endpoint_id: &str,
    timeout_duration: Duration,
) {
    let endpoint_id = Uuid::parse_str(endpoint_id).expect("endpoint id should be UUID");
    let start = Instant::now();
    loop {
        let snapshots = load_manager.snapshots().await;
        if snapshots
            .iter()
            .any(|snapshot| snapshot.endpoint_id == endpoint_id && snapshot.active_requests > 0)
        {
            return;
        }

        if start.elapsed() > timeout_duration {
            panic!("timeout waiting for endpoint to become active");
        }

        sleep(Duration::from_millis(10)).await;
    }
}

async fn mark_endpoint_busy(load_manager: &LoadManager, endpoint_id: &str) {
    let node_id = Uuid::parse_str(endpoint_id).expect("endpoint id should be UUID");
    load_manager
        .record_metrics(MetricsUpdate {
            node_id,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 1,
            average_response_time_ms: None,
            initializing: false,
            ready_models: None,
        })
        .await
        .expect("recording metrics should succeed");
}
