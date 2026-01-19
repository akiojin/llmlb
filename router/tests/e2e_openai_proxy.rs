//! E2E: 実際にHTTP経由でルーターとスタブノードを起動し、
//! OpenAI互換APIのリクエスト・エラー・ストリーミングを検証する。

use std::{sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use llm_router_common::protocol::{ChatRequest, GenerateRequest};
use reqwest::{header, Client};
use serde_json::{json, Value};
use tokio::time::sleep;

#[path = "support/mod.rs"]
mod support;

use support::{
    http::{spawn_router, TestServer},
    router::{
        approve_node, create_test_api_key, register_node, spawn_test_router,
        spawn_test_router_with_db,
    },
};

#[derive(Clone)]
struct NodeStubState {
    chat_response: Value,
    chat_stream_payload: String,
    generate_response: Value,
    generate_stream_payload: String,
}

async fn spawn_node_stub(state: NodeStubState) -> TestServer {
    let shared_state = Arc::new(state);
    let router = Router::new()
        .route("/v1/chat/completions", post(node_chat_handler))
        .route("/v1/completions", post(node_generate_handler))
        .route(
            "/v1/models",
            axum::routing::get(|| async {
                axum::Json(serde_json::json!({"data": [{"id": "gpt-oss-20b"}], "object": "list"}))
            }),
        )
        .with_state(shared_state);

    spawn_router(router).await
}

async fn register_and_approve(router: &TestServer, node_stub: &TestServer) -> Value {
    let register_response = register_node(router.addr(), node_stub.addr())
        .await
        .expect("node registration should succeed");
    assert!(
        register_response.status().is_success(),
        "registration should return success"
    );
    let body: Value = register_response
        .json()
        .await
        .expect("register response should be json");
    let node_id = body["runtime_id"].as_str().expect("node_id should exist");

    let approve_response = approve_node(router.addr(), node_id)
        .await
        .expect("approve request should succeed");
    assert_eq!(approve_response.status(), reqwest::StatusCode::OK);

    body
}

async fn node_chat_handler(
    State(state): State<Arc<NodeStubState>>,
    Json(request): Json<ChatRequest>,
) -> Response {
    if request.model == "missing-model" {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("model not found"))
            .unwrap();
    }

    if request.stream {
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .body(Body::from(state.chat_stream_payload.clone()))
            .unwrap();
    }

    Json(&state.chat_response).into_response()
}

async fn node_generate_handler(
    State(state): State<Arc<NodeStubState>>,
    Json(request): Json<GenerateRequest>,
) -> Response {
    if request.model == "missing-model" {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("model not loaded"))
            .unwrap();
    }

    if request.stream {
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/x-ndjson")
            .body(Body::from(state.generate_stream_payload.clone()))
            .unwrap();
    }

    Json(state.generate_response.clone()).into_response()
}

#[tokio::test]
#[ignore = "TDD RED: Mock node server health check issue"]
async fn openai_proxy_end_to_end_updates_dashboard_history() {
    let node_stub = spawn_node_stub(NodeStubState {
        // OpenAI互換形式のレスポンス
        chat_response: json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello from node"
                },
                "finish_reason": "stop"
            }]
        }),
        chat_stream_payload: "data: {\"choices\":[{\"delta\":{\"content\":\"Hello stream\"}}]}\n\n"
            .to_string(),
        generate_response: json!({
            "response": "generated text",
            "done": true
        }),
        generate_stream_payload: "{\"response\":\"chunk-1\"}\n{\"response\":\"chunk-2\"}\n"
            .to_string(),
    })
    .await;

    let router = spawn_test_router().await;

    register_and_approve(&router, &node_stub).await;

    let client = Client::new();

    // 正常系チャット（OpenAI互換API）
    let chat_response = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&ChatRequest {
            model: "gpt-oss-20b".into(),
            messages: vec![llm_router_common::protocol::ChatMessage {
                role: "user".into(),
                content: "hello?".into(),
            }],
            stream: false,
        })
        .send()
        .await
        .expect("chat request should succeed");
    assert_eq!(chat_response.status(), reqwest::StatusCode::OK);
    let chat_payload: Value = chat_response.json().await.expect("chat json response");
    assert_eq!(
        chat_payload["choices"][0]["message"]["content"],
        "Hello from node"
    );

    // ストリーミングチャット（OpenAI互換API）
    let streaming_response = client
        .post(format!("http://{}/v1/chat/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&ChatRequest {
            model: "gpt-oss-20b".into(),
            messages: vec![llm_router_common::protocol::ChatMessage {
                role: "user".into(),
                content: "stream?".into(),
            }],
            stream: true,
        })
        .send()
        .await
        .expect("streaming chat request should succeed");
    assert_eq!(streaming_response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        streaming_response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok()),
        Some("text/event-stream")
    );
    let streaming_body = streaming_response
        .text()
        .await
        .expect("streaming chat body");
    assert!(
        streaming_body.contains("Hello stream"),
        "expected streaming payload to contain node content"
    );

    // 生成API正常系（OpenAI互換API）
    let generate_response = client
        .post(format!("http://{}/v1/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&GenerateRequest {
            model: "gpt-oss-20b".into(),
            prompt: "write something".into(),
            stream: false,
        })
        .send()
        .await
        .expect("generate request should succeed");
    assert_eq!(generate_response.status(), reqwest::StatusCode::OK);
    let generate_payload: Value = generate_response
        .json()
        .await
        .expect("generate json response");
    assert_eq!(generate_payload["response"], "generated text");

    // 生成APIエラーケース（OpenAI互換API）
    let missing_model_response = client
        .post(format!("http://{}/v1/completions", router.addr()))
        .header("x-api-key", "sk_debug")
        .json(&GenerateRequest {
            model: "missing-model".into(),
            prompt: "fail please".into(),
            stream: false,
        })
        .send()
        .await
        .expect("missing model request should respond");
    assert_eq!(
        missing_model_response.status(),
        reqwest::StatusCode::BAD_REQUEST
    );

    // 集計が反映されるまでポーリング（CIのタイミング揺らぎ対策）
    let mut success = 0u64;
    let mut error = 0u64;
    for _ in 0..20 {
        let history = client
            .get(format!(
                "http://{}/v0/dashboard/request-history",
                router.addr()
            ))
            .header("authorization", "Bearer sk_debug")
            .send()
            .await
            .expect("request history endpoint should respond")
            .json::<Value>()
            .await
            .expect("history payload should be valid JSON");

        assert!(
            history.is_array(),
            "request history payload should be an array"
        );
        if let Some(latest) = history.as_array().and_then(|a| a.last()) {
            success = latest["success"].as_u64().unwrap_or_default();
            error = latest["error"].as_u64().unwrap_or_default();
            if success >= 1 && error >= 1 {
                break;
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(
        success >= 1,
        "expected at least one successful request recorded, got {success}"
    );
    assert!(
        error >= 1,
        "expected at least one failed request recorded, got {error}"
    );

    router.stop().await;
    node_stub.stop().await;
}

/// SPEC-66555000: Endpoints APIを使用してモデルを登録しテストする
#[tokio::test]
async fn openai_v1_models_list_with_registered_node() {
    use support::router::register_responses_endpoint;

    let node_stub = spawn_node_stub(NodeStubState {
        chat_response: json!({
            "message": {"role": "assistant", "content": "Hello"},
            "done": true
        }),
        chat_stream_payload: "".to_string(),
        generate_response: json!({}),
        generate_stream_payload: "".to_string(),
    })
    .await;

    let (router, db_pool) = spawn_test_router_with_db().await;

    // SPEC-66555000: Endpoints API経由でエンドポイントを登録＆モデル同期
    let _ = register_responses_endpoint(router.addr(), node_stub.addr(), "gpt-oss-20b").await;

    // APIキーを取得
    let api_key = create_test_api_key(router.addr(), &db_pool).await;

    let client = Client::new();

    // GET /v1/models
    let models_response = client
        .get(format!("http://{}/v1/models", router.addr()))
        .header("authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(models_response.status(), reqwest::StatusCode::OK);

    let models_payload: Value = models_response.json().await.expect("models json response");

    assert!(
        models_payload.get("data").is_some(),
        "Response must have 'data' field"
    );
    assert!(
        models_payload["data"].is_array(),
        "'data' field must be an array"
    );
    assert_eq!(
        models_payload["object"].as_str(),
        Some("list"),
        "'object' field must be 'list'"
    );

    router.stop().await;
    node_stub.stop().await;
}

/// SPEC-66555000: Endpoints APIを使用してモデルを登録しテストする
#[tokio::test]
async fn openai_v1_models_get_specific() {
    use support::router::register_responses_endpoint;

    let node_stub = spawn_node_stub(NodeStubState {
        chat_response: json!({
            "message": {"role": "assistant", "content": "Hello"},
            "done": true
        }),
        chat_stream_payload: "".to_string(),
        generate_response: json!({}),
        generate_stream_payload: "".to_string(),
    })
    .await;

    let (router, db_pool) = spawn_test_router_with_db().await;

    // SPEC-66555000: Endpoints API経由でエンドポイントを登録＆モデル同期
    let endpoint_id =
        register_responses_endpoint(router.addr(), node_stub.addr(), "gpt-oss-20b").await;
    assert!(
        endpoint_id.is_ok(),
        "Endpoint registration should succeed: {:?}",
        endpoint_id
    );

    // APIキーを取得
    let api_key = create_test_api_key(router.addr(), &db_pool).await;

    let client = Client::new();

    // GET /v1/models/gpt-oss-20b - エンドポイントがモデルを報告しているので発見される
    let model_response = client
        .get(format!("http://{}/v1/models/gpt-oss-20b", router.addr()))
        .header("authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("model request should succeed");

    // エンドポイントがこのモデルを報告しているため、200が返る
    assert_eq!(model_response.status(), reqwest::StatusCode::OK);

    router.stop().await;
    node_stub.stop().await;
}

/// SPEC-66555000: Endpoints APIを使用してモデルを登録しテストする
#[tokio::test]
async fn openai_v1_models_not_found() {
    use support::router::register_responses_endpoint;

    let node_stub = spawn_node_stub(NodeStubState {
        chat_response: json!({
            "message": {"role": "assistant", "content": "Hello"},
            "done": true
        }),
        chat_stream_payload: "".to_string(),
        generate_response: json!({}),
        generate_stream_payload: "".to_string(),
    })
    .await;

    let (router, db_pool) = spawn_test_router_with_db().await;

    // SPEC-66555000: Endpoints API経由でエンドポイントを登録＆モデル同期
    let _ = register_responses_endpoint(router.addr(), node_stub.addr(), "gpt-oss-20b").await;

    // APIキーを取得
    let api_key = create_test_api_key(router.addr(), &db_pool).await;

    let client = Client::new();

    // GET /v1/models/non-existent-model
    let model_response = client
        .get(format!(
            "http://{}/v1/models/non-existent-model",
            router.addr()
        ))
        .header("authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("model request should succeed");

    assert_eq!(model_response.status(), reqwest::StatusCode::NOT_FOUND);

    router.stop().await;
    node_stub.stop().await;
}
