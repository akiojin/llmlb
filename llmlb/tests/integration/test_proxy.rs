//! Integration Test: LLM runtimeプロキシ
//!
//! リクエスト振り分け → LLM runtime転送 → レスポンス返却
//!
//! SPEC #585 FR-028 CP-1: `/v1/chat/completions` 受信 → エンドポイント選択 → 上流転送
//! → ストリーミング応答 → TPS 計測・EMA 更新 を、プロキシ経由（HTTP のみ）で検証する。
//! `#[ignore]` の 2 本は T036-T039 向けの RED プレースホルダーで、実装後に GREEN になる。

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

use crate::support::{
    http::{spawn_lb, TestServer},
    lb::{register_responses_endpoint, spawn_test_lb},
};

const TPS_MODEL: &str = "cp1-tps-model";
/// EMA の平滑化係数（負荷分散戦略: TPS 優先、α=0.2）
const TPS_EMA_ALPHA: f64 = 0.2;
/// 上流スタブが SSE イベントを送る間隔。ストリーム完走までの実時間を計測可能な長さにする
const SSE_EVENT_INTERVAL: Duration = Duration::from_millis(20);
/// 上流スタブが 1 応答で送る SSE イベント数（content / usage / [DONE]）
const SSE_EVENT_COUNT: u32 = 3;

/// ストリーミング応答ごとに申告する出力トークン数を順に返す上流スタブ
struct StreamingUpstreamState {
    completion_tokens: Vec<u64>,
    served: AtomicUsize,
}

async fn spawn_streaming_upstream(completion_tokens: Vec<u64>) -> TestServer {
    let app = Router::new()
        .route("/v1/models", get(upstream_models_handler))
        .route(
            "/v1/chat/completions",
            post(upstream_streaming_chat_handler),
        )
        .with_state(Arc::new(StreamingUpstreamState {
            completion_tokens,
            served: AtomicUsize::new(0),
        }));

    spawn_lb(app).await
}

async fn upstream_models_handler() -> impl IntoResponse {
    Json(json!({
        "object": "list",
        "data": [{"id": TPS_MODEL, "object": "model", "created": 0, "owned_by": "cp1-upstream"}]
    }))
}

async fn upstream_streaming_chat_handler(
    State(state): State<Arc<StreamingUpstreamState>>,
    Json(payload): Json<Value>,
) -> Response {
    if payload["stream"].as_bool() != Some(true) {
        return (StatusCode::BAD_REQUEST, "stream must be true").into_response();
    }

    let index = state.served.fetch_add(1, Ordering::SeqCst);
    let Some(&completion_tokens) = state.completion_tokens.get(index) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected extra request",
        )
            .into_response();
    };

    let events = vec![
        format!(
            "data: {}\n\n",
            json!({
                "id": "chatcmpl-cp1",
                "object": "chat.completion.chunk",
                "model": TPS_MODEL,
                "choices": [{
                    "index": 0,
                    "delta": {"role": "assistant", "content": "hello"},
                    "finish_reason": null
                }]
            })
        ),
        format!(
            "data: {}\n\n",
            json!({
                "id": "chatcmpl-cp1",
                "object": "chat.completion.chunk",
                "model": TPS_MODEL,
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                "usage": {
                    "prompt_tokens": 5,
                    "completion_tokens": completion_tokens,
                    "total_tokens": 5 + completion_tokens
                }
            })
        ),
        "data: [DONE]\n\n".to_string(),
    ];
    let stream = futures::stream::iter(events).then(|event| async move {
        tokio::time::sleep(SSE_EVENT_INTERVAL).await;
        Ok::<_, std::io::Error>(event)
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// プロキシ経由でストリーミング応答を最後まで読み切り、SSE 本文を返す
async fn stream_chat_through_proxy(client: &Client, lb_addr: SocketAddr) -> String {
    let resp = client
        .post(format!("http://{}/v1/chat/completions", lb_addr))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": TPS_MODEL,
            "messages": [{"role": "user", "content": "ping"}],
            "stream": true
        }))
        .send()
        .await
        .expect("streaming chat request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "proxy must pass the upstream SSE stream through, got content-type {content_type:?}"
    );

    resp.text().await.expect("streaming chat body")
}

async fn fetch_model_tps(client: &Client, lb_addr: SocketAddr, endpoint_id: &str) -> Vec<Value> {
    let resp = client
        .get(format!(
            "http://{}/api/endpoints/{}/model-tps",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("model-tps request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    resp.json().await.expect("model-tps json")
}

/// 統計記録はストリーム完走後に非同期で行われるため、完了数が揃うまで待つ
async fn wait_for_model_tps(
    client: &Client,
    lb_addr: SocketAddr,
    endpoint_id: &str,
    request_count: u64,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let entries = fetch_model_tps(client, lb_addr, endpoint_id).await;
        if let Some(entry) = entries.iter().find(|entry| {
            entry["model_id"] == TPS_MODEL && entry["request_count"].as_u64() == Some(request_count)
        }) {
            return entry.clone();
        }

        if Instant::now() > deadline {
            panic!(
                "Timed out waiting for model-tps request_count={request_count}; last entries: {entries:?}"
            );
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn assert_tps_close(actual: f64, expected: f64, context: &str) {
    let tolerance = expected.abs() * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{context}: expected {expected}, got {actual}"
    );
}

/// FR-028 CP-1: プロキシ経由のストリーミング応答で TPS が計測され、EMA として更新される
#[tokio::test]
async fn test_streaming_proxy_measures_tps_and_updates_ema() {
    const FIRST_TOKENS: u64 = 40;
    const SECOND_TOKENS: u64 = 400;

    let lb = spawn_test_lb().await;
    let client = Client::new();
    let upstream = spawn_streaming_upstream(vec![FIRST_TOKENS, SECOND_TOKENS]).await;
    let endpoint_id = register_responses_endpoint(lb.addr(), upstream.addr(), TPS_MODEL)
        .await
        .expect("register upstream endpoint");

    assert!(
        fetch_model_tps(&client, lb.addr(), &endpoint_id)
            .await
            .is_empty(),
        "TPS must be unmeasured before any inference request"
    );

    // 1 回目: 初回計測値がそのまま TPS になる
    let body = stream_chat_through_proxy(&client, lb.addr()).await;
    assert!(body.contains(r#""content":"hello""#), "body: {body}");
    assert!(body.contains("data: [DONE]"), "body: {body}");

    let first = wait_for_model_tps(&client, lb.addr(), &endpoint_id, 1).await;
    assert_eq!(first["api_kind"], "chat_completions");
    assert_eq!(first["source"], "production");
    assert_eq!(first["total_output_tokens"], FIRST_TOKENS);

    let first_duration_ms = first["average_duration_ms"]
        .as_f64()
        .expect("average_duration_ms after first request");
    let min_stream_ms = (SSE_EVENT_INTERVAL * SSE_EVENT_COUNT).as_millis() as f64;
    assert!(
        first_duration_ms >= min_stream_ms,
        "duration must cover the whole stream ({min_stream_ms} ms), got {first_duration_ms} ms"
    );

    let first_tps = first["tps"].as_f64().expect("tps after first request");
    assert_tps_close(
        first_tps,
        FIRST_TOKENS as f64 / (first_duration_ms / 1000.0),
        "first measurement",
    );

    // 2 回目: 直近値での上書きではなく EMA（α=0.2）で更新される
    stream_chat_through_proxy(&client, lb.addr()).await;

    let second = wait_for_model_tps(&client, lb.addr(), &endpoint_id, 2).await;
    assert_eq!(second["total_output_tokens"], FIRST_TOKENS + SECOND_TOKENS);

    let total_duration_ms = second["average_duration_ms"]
        .as_f64()
        .expect("average_duration_ms after second request")
        * 2.0;
    let second_duration_ms = total_duration_ms - first_duration_ms;
    let second_instant_tps = SECOND_TOKENS as f64 / (second_duration_ms / 1000.0);
    let expected_ema = TPS_EMA_ALPHA * second_instant_tps + (1.0 - TPS_EMA_ALPHA) * first_tps;

    assert_tps_close(
        second["tps"].as_f64().expect("tps after second request"),
        expected_ema,
        "EMA after second measurement",
    );
}

const ENGINE_MODEL: &str = "gpt-oss:20b";
const CANONICAL_MODEL: &str = "openai/gpt-oss-20b";
const UNMAPPED_MODEL: &str = "us032-unmapped:latest";

/// 受信した推論リクエストのボディを記録する上流スタブ
#[derive(Default)]
struct RecordingUpstreamState {
    chat_bodies: Mutex<Vec<Value>>,
    embedding_bodies: Mutex<Vec<Value>>,
}

/// 上流スタブが公開するモデル ID
fn recording_upstream_model_ids() -> [String; 3] {
    [
        ENGINE_MODEL.to_string(),
        UNMAPPED_MODEL.to_string(),
        llmlb::config::get_default_embedding_model(),
    ]
}

/// `/api/tags` に応答するため Ollama として判別される上流スタブを起動する
async fn spawn_recording_ollama_upstream() -> (TestServer, Arc<RecordingUpstreamState>) {
    let state = Arc::new(RecordingUpstreamState::default());
    let app = Router::new()
        .route("/api/tags", get(recording_tags_handler))
        .route("/v1/models", get(recording_models_handler))
        .route("/v1/chat/completions", post(recording_chat_handler))
        .route("/v1/embeddings", post(recording_embeddings_handler))
        .with_state(state.clone());

    (spawn_lb(app).await, state)
}

async fn recording_tags_handler() -> impl IntoResponse {
    let models: Vec<Value> = recording_upstream_model_ids()
        .iter()
        .map(|id| json!({"name": id}))
        .collect();
    Json(json!({"models": models}))
}

async fn recording_models_handler() -> impl IntoResponse {
    let data: Vec<Value> = recording_upstream_model_ids()
        .iter()
        .map(|id| json!({"id": id, "object": "model"}))
        .collect();
    Json(json!({"object": "list", "data": data}))
}

async fn recording_chat_handler(
    State(state): State<Arc<RecordingUpstreamState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    state.chat_bodies.lock().unwrap().push(body);
    Json(json!({
        "id": "chatcmpl-us032",
        "object": "chat.completion",
        "model": "upstream-reported-model",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "pong"},
            "finish_reason": "stop"
        }]
    }))
}

async fn recording_embeddings_handler(
    State(state): State<Arc<RecordingUpstreamState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    state.embedding_bodies.lock().unwrap().push(body);
    Json(json!({
        "object": "list",
        "model": "upstream-reported-model",
        "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2]}]
    }))
}

/// SPEC #575 US-032 / AS-032-6: `/v1/chat/completions` が上流へ送るボディの `model` は、
/// エンドポイント向けモデル名の解決規則（FR-041）どおりである
#[tokio::test]
async fn test_chat_completions_sends_resolved_model_to_upstream() {
    let lb = spawn_test_lb().await;
    let client = Client::new();
    let (upstream, upstream_state) = spawn_recording_ollama_upstream().await;
    register_responses_endpoint(lb.addr(), upstream.addr(), "us032")
        .await
        .expect("register upstream endpoint");

    // (要求したモデル名, 上流が受け取るべきモデル名)
    let cases = [
        // エンドポイントが公開するモデル ID と一致
        (ENGINE_MODEL, ENGINE_MODEL),
        // canonical 名はエンジン固有 ID に戻して転送する
        (CANONICAL_MODEL, ENGINE_MODEL),
        // 内蔵マッピングに無いモデル ID はそのまま転送する
        (UNMAPPED_MODEL, UNMAPPED_MODEL),
    ];

    for (requested, expected_upstream) in cases {
        // 同一ホストの別プロセスがスタブのポートへ要求を送りうるため、
        // 回数ではなく本テストが送ったプロンプトで照合する
        let prompt = format!("us032-ping-{requested}");
        let resp = client
            .post(format!("http://{}/v1/chat/completions", lb.addr()))
            .header("x-api-key", "sk_debug")
            .json(&json!({
                "model": requested,
                "messages": [{"role": "user", "content": prompt}],
                "stream": false
            }))
            .send()
            .await
            .expect("chat request");
        assert_eq!(resp.status(), reqwest::StatusCode::OK, "model: {requested}");

        let body: Value = resp.json().await.expect("chat response body");
        assert_eq!(
            body["model"], requested,
            "client must see the model name it requested"
        );

        let upstream_models: Vec<Value> = upstream_state
            .chat_bodies
            .lock()
            .unwrap()
            .iter()
            .filter(|body| body["messages"][0]["content"] == prompt.as_str())
            .map(|body| body["model"].clone())
            .collect();
        assert_eq!(
            upstream_models,
            vec![json!(expected_upstream)],
            "upstream model for requested model {requested}"
        );
    }
}

/// SPEC #575 US-032: `/v1/embeddings` はリクエストに `model` が無くても、
/// 既定モデルの解決結果を上流のボディへ書き込む
#[tokio::test]
async fn test_embeddings_without_model_sends_default_model_to_upstream() {
    const INPUT: &str = "us032-embedding-without-model";

    let lb = spawn_test_lb().await;
    let client = Client::new();
    let (upstream, upstream_state) = spawn_recording_ollama_upstream().await;
    register_responses_endpoint(lb.addr(), upstream.addr(), "us032-embeddings")
        .await
        .expect("register upstream endpoint");

    let resp = client
        .post(format!("http://{}/v1/embeddings", lb.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({"input": INPUT}))
        .send()
        .await
        .expect("embeddings request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let upstream_models: Vec<Value> = upstream_state
        .embedding_bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|body| body["input"] == INPUT)
        .map(|body| body["model"].clone())
        .collect();
    assert_eq!(
        upstream_models,
        vec![json!(llmlb::config::get_default_embedding_model())]
    );
}

#[tokio::test]
#[ignore = "TDD RED: LLM runtimeプロキシ未実装"]
async fn test_proxy_request_to_single_node() {
    // Arrange: Routerサーバー起動、1台のノード登録、モックLLM runtime起動
    // let lb = start_test_lb().await;
    // let mock_runtime = start_mock_runtime().await;
    // register_test_node(&lb, mock_runtime.url()).await;

    // Act: チャットリクエスト送信
    // let request = json!({
    //     "model": "llama2",
    //     "messages": [{"role": "user", "content": "Hello"}]
    // });
    // let response = app.post("/v1/chat/completions", request).await;

    // Assert: 正常にレスポンスが返された
    // assert_eq!(response.status(), 200);
    // let body: serde_json::Value = response.json();
    // assert!(body["message"].is_object());

    // TODO: T036-T039で実装後にアンコメント
    panic!("RED: LLM runtimeプロキシが未実装");
}

#[tokio::test]
#[ignore = "TDD RED: LLM runtimeプロキシ未実装"]
async fn test_proxy_no_nodes_returns_503() {
    // Arrange: Routerサーバー起動（ノード未登録）
    // let lb = start_test_lb().await;

    // Act: チャットリクエスト送信
    // let request = json!({
    //     "model": "llama2",
    //     "messages": [{"role": "user", "content": "Hello"}]
    // });
    // let response = app.post("/v1/chat/completions", request).await;

    // Assert: 503 Service Unavailable
    // assert_eq!(response.status(), 503);

    // TODO: T036-T039で実装後にアンコメント
    panic!("RED: LLM runtimeプロキシが未実装");
}
