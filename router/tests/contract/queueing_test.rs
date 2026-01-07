//! Contract Test: Router request queueing

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

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

use crate::support::{
    http::{spawn_router, TestServer},
    router::{approve_node_from_register_response, register_node, spawn_test_router},
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
    let router = Router::new()
        .route("/v1/chat/completions", post(chat_handler))
        .route("/v1/models", get(models_handler))
        .route("/api/tags", get(tags_handler))
        .route("/api/health", post(|| async { StatusCode::OK }))
        .with_state(Arc::new(state));

    spawn_router(router).await
}

async fn chat_handler(State(state): State<Arc<QueueStubState>>, Json(_): Json<serde_json::Value>) -> impl IntoResponse {
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
    (StatusCode::OK, Json(json!({"data": [{"id": "test-model", "object": "model"}]}))).into_response()
}

async fn tags_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"models": []}))).into_response()
}

fn set_queue_env(max: usize, timeout_secs: u64) {
    std::env::set_var("LLM_ROUTER_QUEUE_MAX", max.to_string());
    std::env::set_var("LLM_ROUTER_QUEUE_TIMEOUT_SECS", timeout_secs.to_string());
}

fn clear_queue_env() {
    std::env::remove_var("LLM_ROUTER_QUEUE_MAX");
    std::env::remove_var("LLM_ROUTER_QUEUE_TIMEOUT_SECS");
}

fn chat_payload() -> serde_json::Value {
    json!({
        "model": "gpt-oss-20b",
        "messages": [{"role": "user", "content": "ping"}]
    })
}

#[tokio::test]
#[serial]
async fn queued_request_waits_and_sets_header() {
    set_queue_env(2, 2);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), stub.addr())
        .await
        .expect("register node must succeed");
    let (status, _body) =
        approve_node_from_register_response(router.addr(), register_response)
            .await
            .expect("approve node must succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
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

    let second = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&chat_payload())
                .send()
                .await
                .expect("second request should be sent")
        }
    });

    sleep(Duration::from_millis(100)).await;
    assert_eq!(stub_state.request_count.load(Ordering::SeqCst), 1);

    stub_state.release_first.notify_waiters();

    let first_resp = first.await.expect("first join should succeed");
    assert_eq!(first_resp.status(), ReqStatusCode::OK);

    let second_resp = second.await.expect("second join should succeed");
    assert_eq!(second_resp.status(), ReqStatusCode::OK);
    let queued_header = second_resp
        .headers()
        .get("x-queue-status")
        .and_then(|v| v.to_str().ok());
    assert_eq!(queued_header, Some("queued"));

    let wait_ms = second_resp
        .headers()
        .get("x-queue-wait-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    assert!(wait_ms > 0);

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn queue_full_returns_429_with_retry_after() {
    set_queue_env(1, 2);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), stub.addr())
        .await
        .expect("register node must succeed");
    let (status, _body) =
        approve_node_from_register_response(router.addr(), register_response)
            .await
            .expect("approve node must succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
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

    let second = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
        async move {
            client
                .post(format!("http://{addr}/v1/chat/completions"))
                .header("x-api-key", "sk_debug")
                .json(&chat_payload())
                .send()
                .await
                .expect("second request should be sent")
        }
    });

    let third_resp = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("third request should be sent");

    assert_eq!(third_resp.status(), ReqStatusCode::TOO_MANY_REQUESTS);
    assert!(third_resp.headers().get("retry-after").is_some());

    stub_state.release_first.notify_waiters();

    let _ = first.await.expect("first join should succeed");
    let second_resp = second.await.expect("second join should succeed");
    assert_eq!(second_resp.status(), ReqStatusCode::OK);

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn queue_timeout_returns_504() {
    set_queue_env(1, 0);

    let stub_state = QueueStubState::new(true, "node-a");
    let stub = spawn_queue_stub(stub_state.clone()).await;
    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), stub.addr())
        .await
        .expect("register node must succeed");
    let (status, _body) =
        approve_node_from_register_response(router.addr(), register_response)
            .await
            .expect("approve node must succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
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

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::GATEWAY_TIMEOUT);

    stub_state.release_first.notify_waiters();
    let _ = first.await.expect("first join should succeed");

    clear_queue_env();
}

#[tokio::test]
#[serial]
async fn routes_to_idle_node_when_one_busy() {
    set_queue_env(2, 2);

    let busy_state = QueueStubState::new(true, "node-a");
    let idle_state = QueueStubState::new(false, "node-b");

    let busy_stub = spawn_queue_stub(busy_state.clone()).await;
    let idle_stub = spawn_queue_stub(idle_state.clone()).await;

    let router = spawn_test_router().await;

    let register_response = register_node(router.addr(), busy_stub.addr())
        .await
        .expect("register node must succeed");
    let (status, _body) =
        approve_node_from_register_response(router.addr(), register_response)
            .await
            .expect("approve node must succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

    let register_response = register_node(router.addr(), idle_stub.addr())
        .await
        .expect("register node must succeed");
    let (status, _body) =
        approve_node_from_register_response(router.addr(), register_response)
            .await
            .expect("approve node must succeed");
    assert_eq!(status, ReqStatusCode::CREATED);

    let client = Client::new();

    let first = tokio::spawn({
        let client = client.clone();
        let addr = router.addr();
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

    let second_resp = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&chat_payload())
        .send()
        .await
        .expect("second request should be sent");

    assert_eq!(second_resp.status(), ReqStatusCode::OK);
    let payload: serde_json::Value = second_resp.json().await.expect("valid json response");
    assert_eq!(payload.get("id").and_then(|v| v.as_str()), Some("node-b"));
    assert_eq!(idle_state.request_count.load(Ordering::SeqCst), 1);

    busy_state.release_first.notify_waiters();
    let _ = first.await.expect("first join should succeed");

    clear_queue_env();
}
