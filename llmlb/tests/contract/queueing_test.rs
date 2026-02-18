//! Contract Test: load balancer request queueing

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::json;
use serial_test::serial;
use tokio::sync::Notify;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::spawn_test_lb_with_manager,
};

#[derive(Clone)]
struct QueueStubState {
    request_count: Arc<AtomicUsize>,
    first_started: Arc<Notify>,
    release_first: Arc<Notify>,
    block_first: bool,
    response_label: String,
}

impl QueueStubState {
    fn new(block_first: bool, response_label: &str) -> Self {
        Self {
            request_count: Arc::new(AtomicUsize::new(0)),
            first_started: Arc::new(Notify::new()),
            release_first: Arc::new(Notify::new()),
            block_first,
            response_label: response_label.to_string(),
        }
    }
}

async fn spawn_queue_stub(state: QueueStubState) -> TestServer {
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_lb(app).await
}

async fn chat_handler(
    State(state): State<Arc<QueueStubState>>,
    Json(_): Json<serde_json::Value>,
) -> impl IntoResponse {
    let count = state.request_count.fetch_add(1, Ordering::SeqCst);
    if count == 0 && state.block_first {
        state.first_started.notify_waiters();
        state.release_first.notified().await;
    }

    let body = json!({
        "id": state.response_label,
        "object": "chat.completion",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
    });

    (StatusCode::OK, Json(body)).into_response()
}

// SPEC-93536000: 空のモデルリストは登録拒否されるため、少なくとも1つのモデルを返す
async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({"data": [{"id": "test-model", "object": "model"}]})),
    )
        .into_response()
}

async fn tags_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"models": []}))).into_response()
}

fn set_queue_env(max: usize, timeout_secs: u64) {
    std::env::set_var("LLMLB_QUEUE_MAX", max.to_string());
    std::env::set_var("LLMLB_QUEUE_TIMEOUT_SECS", timeout_secs.to_string());
}

fn clear_queue_env() {
    std::env::remove_var("LLMLB_QUEUE_MAX");
    std::env::remove_var("LLMLB_QUEUE_TIMEOUT_SECS");
}

fn chat_payload() -> serde_json::Value {
    json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "ping"}]
    })
}

#[tokio::test]
#[serial]
async fn concurrent_request_is_forwarded_without_queue_wait() {
    set_queue_env(2, 2);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let (lb, load_manager) = spawn_test_lb_with_manager().await;
    let endpoint_id = register_queue_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = lb.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&chat_payload())
                .send()
                .await
                .expect("first request should be sent")
        }
    });

    timeout(Duration::from_secs(1), stub_state.first_started.notified())
        .await
        .expect("first request should reach node");

    wait_for_endpoint_active(&load_manager, &endpoint_id, Duration::from_secs(1)).await;

    let second_started = Instant::now();
    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("second request should be sent");
    let second_elapsed = second_started.elapsed();

    assert_eq!(second_resp.status(), ReqStatusCode::OK);
    assert!(
        second_elapsed < Duration::from_secs(1),
        "second request should not wait in queue"
    );
    assert!(second_resp.headers().get("x-queue-status").is_none());
    assert!(second_resp.headers().get("x-queue-wait-ms").is_none());
    assert_eq!(stub_state.request_count.load(Ordering::SeqCst), 2);

    stub_state.release_first.notify_waiters();

    let first_resp = first.await.expect("first join should succeed");
    assert_eq!(first_resp.status(), ReqStatusCode::OK);

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn queue_full_config_does_not_block_round_robin_forwarding() {
    set_queue_env(0, 2);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let (lb, load_manager) = spawn_test_lb_with_manager().await;
    let endpoint_id = register_queue_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();
    let payload = chat_payload();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = lb.addr();
        let payload = payload.clone();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&payload)
                .send()
                .await
                .expect("first request should be sent")
        }
    });

    timeout(Duration::from_secs(1), stub_state.first_started.notified())
        .await
        .expect("first request should reach node");

    wait_for_endpoint_active(&load_manager, &endpoint_id, Duration::from_secs(1)).await;

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::OK);
    assert!(second_resp.headers().get("retry-after").is_none());

    stub_state.release_first.notify_waiters();

    let _ = first.await.expect("first join should succeed");

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn queue_timeout_config_does_not_trigger_when_round_robin_can_route() {
    set_queue_env(1, 0);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let (lb, load_manager) = spawn_test_lb_with_manager().await;
    let endpoint_id = register_queue_endpoint(lb.addr(), stub.addr())
        .await
        .expect("register endpoint must succeed");

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = lb.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&chat_payload())
                .send()
                .await
                .expect("first request should be sent")
        }
    });

    timeout(Duration::from_secs(1), stub_state.first_started.notified())
        .await
        .expect("first request should reach node");

    wait_for_endpoint_active(&load_manager, &endpoint_id, Duration::from_secs(1)).await;

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::OK);

    stub_state.release_first.notify_waiters();
    let _ = first.await.expect("first join should succeed");

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn routes_to_online_node_when_one_busy() {
    set_queue_env(2, 2);

    let busy_state = QueueStubState::new(true, "node-a");
    let idle_state = QueueStubState::new(false, "node-b");

    let busy_stub = spawn_queue_stub(busy_state.clone()).await;
    let idle_stub = spawn_queue_stub(idle_state.clone()).await;

    let (lb, load_manager) = spawn_test_lb_with_manager().await;
    let busy_endpoint_id = register_queue_endpoint(lb.addr(), busy_stub.addr())
        .await
        .expect("register busy endpoint must succeed");

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = lb.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&chat_payload())
                .send()
                .await
                .expect("first request should be sent")
        }
    });

    timeout(Duration::from_secs(1), busy_state.first_started.notified())
        .await
        .expect("busy node should receive first request");

    wait_for_endpoint_active(&load_manager, &busy_endpoint_id, Duration::from_secs(1)).await;

    let _idle_endpoint_id = register_queue_endpoint(lb.addr(), idle_stub.addr())
        .await
        .expect("register idle endpoint must succeed");
    sleep(Duration::from_millis(50)).await;

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::OK);
    let payload: serde_json::Value = second_resp.json().await.expect("valid json response");
    let responder = payload.get("id").and_then(|v| v.as_str());
    assert!(
        responder == Some("node-a") || responder == Some("node-b"),
        "response should come from an online endpoint"
    );
    assert!(
        busy_state.request_count.load(Ordering::SeqCst)
            + idle_state.request_count.load(Ordering::SeqCst)
            >= 2
    );

    busy_state.release_first.notify_waiters();
    let _ = first.await.expect("first join should succeed");

    clear_queue_env();
}

async fn register_queue_endpoint(
    lb_addr: std::net::SocketAddr,
    stub_addr: std::net::SocketAddr,
) -> reqwest::Result<String> {
    let client = Client::new();

    let create_response = client
        .post(format!("http://{}/api/endpoints", lb_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": format!("Queue Stub - {}", stub_addr),
            "base_url": format!("http://{}", stub_addr),
            "health_check_interval_secs": 30
        }))
        .send()
        .await?;

    let create_body: serde_json::Value = create_response.json().await.unwrap_or_default();
    let endpoint_id = create_body["id"].as_str().unwrap_or_default().to_string();

    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await?;

    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await?;

    Ok(endpoint_id)
}

async fn wait_for_endpoint_active(
    load_manager: &llmlb::balancer::LoadManager,
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
