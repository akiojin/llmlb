//! Contract Test: 互換APIの未知フィールド透過（Issue #775 / SPEC #575）
//!
//! `/v1/responses` / `/v1/chat/completions` / `/v1/completions` / `/v1/embeddings` は
//! リクエストを型付けせずに転送するため、llmlb が知らないフィールドも上流へ到達する。
//! この性質が上流 API への追従性を担保しているので、スタブが受信したボディを記録し、
//! 送信したフィールドが欠落・改変なく届いたことを表明して固定する。
//!
//! これらはリグレッション防止のテストであり、ルートに allow-list や型付き struct を
//! 導入してフィールドを落とす変更が入ると失敗する。

use std::sync::{Arc, Mutex};

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_responses_endpoint, spawn_test_lb},
};
use axum::{
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::{json, Value};
use serial_test::serial;

const MODEL: &str = "test-model";

/// llmlb が一切解釈しない合成キー。allow-list 方式の実装では必ず欠落する。
const PROBE_KEY: &str = "__llmlb_unknown_probe";

fn probe_value() -> Value {
    json!({
        "nested": [1, "two", {"three": true}],
        "null_member": null
    })
}

/// 上流スタブが受信したリクエストボディを、パスごとに記録する。
#[derive(Clone, Default)]
struct RecordingStubState {
    received: Arc<Mutex<Vec<(String, Value)>>>,
}

impl RecordingStubState {
    /// `path` に届いた唯一のリクエストボディを返す。
    fn single_request(&self, path: &str) -> Value {
        let received = self
            .received
            .lock()
            .expect("received lock should not be poisoned");
        let mut matching = received.iter().filter(|(p, _)| p == path);
        let body = matching
            .next()
            .unwrap_or_else(|| panic!("upstream stub must receive a request on {path}"))
            .1
            .clone();
        assert!(
            matching.next().is_none(),
            "upstream stub must receive exactly one request on {path}"
        );
        body
    }
}

async fn spawn_recording_stub() -> (TestServer, RecordingStubState) {
    let state = RecordingStubState::default();
    let app = Router::new()
        .route("/v1/responses", post(recording_handler))
        .route("/v1/chat/completions", post(recording_handler))
        .route("/v1/completions", post(recording_handler))
        .route("/v1/embeddings", post(recording_handler))
        .route("/v1/models", get(models_handler))
        .with_state(state.clone());

    (spawn_lb(app).await, state)
}

async fn recording_handler(
    State(state): State<RecordingStubState>,
    uri: Uri,
    Json(body): Json<Value>,
) -> Response {
    let is_stream = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    state
        .received
        .lock()
        .expect("received lock should not be poisoned")
        .push((uri.path().to_string(), body));

    if is_stream {
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .body(axum::body::Body::from("data: [DONE]\n\n"))
            .expect("stream response should build");
    }

    let payload = match uri.path() {
        "/v1/responses" => json!({"id": "resp_probe", "object": "response", "output": []}),
        "/v1/chat/completions" => json!({
            "id": "chatcmpl-probe",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop"
            }]
        }),
        "/v1/completions" => json!({
            "id": "cmpl-probe",
            "object": "text_completion",
            "choices": [{"text": "ok", "index": 0, "logprobs": null, "finish_reason": "stop"}]
        }),
        _ => json!({
            "object": "list",
            "data": [{"object": "embedding", "index": 0, "embedding": [0.0, 0.1]}]
        }),
    };
    (StatusCode::OK, Json(payload)).into_response()
}

async fn models_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({"object": "list", "data": [{"id": MODEL, "object": "model"}]})),
    )
}

/// `request` を `path` へ送り、上流スタブが受信したボディを返す。
async fn forward_and_capture(path: &str, request: &Value) -> Value {
    let (stub, state) = spawn_recording_stub().await;
    let lb = spawn_test_lb().await;
    register_responses_endpoint(lb.addr(), stub.addr(), MODEL)
        .await
        .expect("register endpoint must succeed");

    let response = Client::new()
        .post(format!("http://{}{}", lb.addr(), path))
        .header("x-api-key", "sk_debug")
        .json(request)
        .send()
        .await
        .expect("request should complete");
    assert_eq!(
        response.status(),
        ReqStatusCode::OK,
        "{path} must be proxied successfully"
    );
    // ストリーミングでもボディを最後まで読み、転送の完了を待つ
    let _ = response
        .bytes()
        .await
        .expect("response body should be read");

    state.single_request(path)
}

/// 送信したフィールドが、1 つ残らず同じ値で上流に届いたことを表明する。
fn assert_all_fields_forwarded(path: &str, sent: &Value, received: &Value) {
    let sent = sent.as_object().expect("sent body must be an object");
    let received = received
        .as_object()
        .unwrap_or_else(|| panic!("{path}: upstream must receive a JSON object"));
    for (key, value) in sent {
        assert_eq!(
            received.get(key),
            Some(value),
            "{path}: field `{key}` must reach the upstream endpoint unchanged"
        );
    }
}

async fn assert_passthrough(path: &str, request: Value) {
    assert!(
        request.get(PROBE_KEY).is_some(),
        "request must carry the synthetic probe key"
    );
    let received = forward_and_capture(path, &request).await;
    assert_all_fields_forwarded(path, &request, &received);
}

#[tokio::test]
#[serial]
async fn responses_forwards_unknown_fields_to_upstream() {
    assert_passthrough(
        "/v1/responses",
        json!({
            "model": MODEL,
            "input": "Hello!",
            PROBE_KEY: probe_value(),
            "reasoning": {"effort": "high", "summary": "auto"},
            "previous_response_id": "resp_previous_123",
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "background": false,
            "tools": [{"type": "web_search"}]
        }),
    )
    .await;
}

#[tokio::test]
#[serial]
async fn responses_streaming_forwards_unknown_fields_to_upstream() {
    assert_passthrough(
        "/v1/responses",
        json!({
            "model": MODEL,
            "input": "Hello!",
            "stream": true,
            PROBE_KEY: probe_value(),
            "reasoning": {"effort": "low"},
            "store": false
        }),
    )
    .await;
}

#[tokio::test]
#[serial]
async fn chat_completions_forwards_unknown_fields_to_upstream() {
    assert_passthrough(
        "/v1/chat/completions",
        json!({
            "model": MODEL,
            "messages": [{"role": "user", "content": "Hello!"}],
            PROBE_KEY: probe_value(),
            "reasoning_effort": "high",
            "verbosity": "low",
            "service_tier": "flex",
            "prediction": {"type": "content", "content": "predicted"}
        }),
    )
    .await;
}

#[tokio::test]
#[serial]
async fn chat_completions_streaming_forwards_unknown_fields_to_upstream() {
    let request = json!({
        "model": MODEL,
        "messages": [{"role": "user", "content": "Hello!"}],
        "stream": true,
        PROBE_KEY: probe_value(),
        "reasoning_effort": "high",
        "stream_options": {"include_obfuscation": false}
    });
    let received = forward_and_capture("/v1/chat/completions", &request).await;

    // llmlb は stream_options.include_usage を注入するため、stream_options だけは
    // 完全一致ではなく「クライアント指定のメンバーが残っていること」を表明する。
    let mut expected = request.clone();
    expected
        .as_object_mut()
        .expect("request must be an object")
        .remove("stream_options");
    assert_all_fields_forwarded("/v1/chat/completions", &expected, &received);
    assert_eq!(
        received["stream_options"]["include_obfuscation"],
        json!(false),
        "client-supplied stream_options members must survive include_usage injection"
    );
    assert_eq!(received["stream_options"]["include_usage"], json!(true));
}

#[tokio::test]
#[serial]
async fn completions_forwards_unknown_fields_to_upstream() {
    assert_passthrough(
        "/v1/completions",
        json!({
            "model": MODEL,
            "prompt": "ping",
            PROBE_KEY: probe_value(),
            "seed": 42,
            "suffix": " pong",
            "logit_bias": {"50256": -100}
        }),
    )
    .await;
}

#[tokio::test]
#[serial]
async fn embeddings_forwards_unknown_fields_to_upstream() {
    assert_passthrough(
        "/v1/embeddings",
        json!({
            "model": MODEL,
            "input": ["Hello!"],
            PROBE_KEY: probe_value(),
            "dimensions": 256,
            "encoding_format": "base64",
            "user": "user-775"
        }),
    )
    .await;
}
