//! Integration Tests: モデル対応ルーティング
//!
//! SPEC-93536000 Phase 6 (6.4-6.9)
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに更新済み。
//! 廃止されたPUSH型登録フロー（/v0/nodes, /v0/health）を使用するテストは#[ignore]。

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use llm_router::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use llm_router_common::protocol::RegisterResponse;
use serial_test::serial;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct TestEnvGuard {
    keys: Vec<&'static str>,
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        for key in &self.keys {
            std::env::remove_var(key);
        }
    }
}

async fn build_test_router() -> (AppState, Router, TestEnvGuard) {
    let temp_dir = std::env::temp_dir().join(format!(
        "model-routing-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    std::env::set_var("AUTH_DISABLED", "true");
    std::env::set_var("LLM_CONVERT_FAKE", "1");

    let guard = TestEnvGuard {
        keys: vec!["LLM_ROUTER_DATA_DIR", "AUTH_DISABLED", "LLM_CONVERT_FAKE"],
    };

    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));
    llm_router::api::models::clear_registered_models(&db_pool)
        .await
        .expect("clear registered models");

    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = "test-secret".to_string();

    let state = AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let router = api::create_router(state.clone());
    (state, router, guard)
}

async fn post_json(router: &Router, path: &str, payload: Value) -> Response {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post_empty(router: &Router, path: &str) -> Response {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn read_json(response: Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("parse json")
}

async fn register_node(router: &Router, mock: &MockServer, machine_name: &str) -> Uuid {
    let payload = json!({
        "machine_name": machine_name,
        "ip_address": mock.address().ip().to_string(),
        "runtime_version": "0.0.0-test",
        "runtime_port": mock.address().port().saturating_sub(1),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
        ],
        "supported_runtimes": [],
    });

    let response = post_json(router, "/v0/nodes", payload).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read register body");
    let register: RegisterResponse = serde_json::from_slice(&body).expect("register json");
    register.node_id
}

async fn approve_node(router: &Router, node_id: Uuid) {
    let response = post_empty(router, &format!("/v0/nodes/{}/approve", node_id)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

async fn send_health(router: &Router, node_id: Uuid, cpu_usage: f32) {
    let payload = json!({
        "node_id": node_id,
        "cpu_usage": cpu_usage,
        "memory_usage": 0.0,
        "active_requests": 0,
        "average_response_time_ms": 1.0,
        "loaded_models": [],
        "loaded_embedding_models": [],
        "loaded_asr_models": [],
        "loaded_tts_models": [],
        "supported_runtimes": [],
        "initializing": false,
        "ready_models": [1, 1],
    });

    let response = post_json(router, "/v0/health", payload).await;
    assert_eq!(response.status(), StatusCode::OK);
}

fn mount_models(mock: &MockServer, models: &[&str]) -> impl std::future::Future<Output = ()> + '_ {
    let data: Vec<Value> = models.iter().map(|m| json!({ "id": m })).collect();
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": data,
        })))
        .mount(mock)
}

fn chat_payload(model: &str, content: &str) -> Value {
    json!({
        "model": model,
        "messages": [
            {"role": "user", "content": content}
        ],
        "stream": false,
    })
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: /v0/nodes, /v0/healthエンドポイントが削除されたため (SPEC-66555000)"]
async fn test_routes_request_to_capable_node() {
    let (_state, router, _guard) = build_test_router().await;

    let node_a = MockServer::start().await;
    let node_b = MockServer::start().await;

    mount_models(&node_a, &["model-a"]).await;
    mount_models(&node_b, &["model-b"]).await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "chat.completion",
                "choices": [{
                    "message": {"role": "assistant", "content": "from-b"},
                    "finish_reason": "stop",
                    "index": 0
                }]
            })),
        )
        .mount(&node_b)
        .await;

    let node_a_id = register_node(&router, &node_a, "node-a").await;
    let node_b_id = register_node(&router, &node_b, "node-b").await;
    approve_node(&router, node_a_id).await;
    approve_node(&router, node_b_id).await;
    send_health(&router, node_a_id, 10.0).await;
    send_health(&router, node_b_id, 10.0).await;

    let response = post_json(&router, "/v1/chat/completions", chat_payload("model-b", "hi")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["choices"][0]["message"]["content"], "from-b");
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: /v0/nodes, /v0/healthエンドポイントが削除されたため (SPEC-66555000)"]
async fn test_request_for_known_model_without_node_returns_503() {
    let (state, router, _guard) = build_test_router().await;

    let node = MockServer::start().await;
    mount_models(&node, &["model-a"]).await;

    let node_id = register_node(&router, &node, "node-a").await;
    approve_node(&router, node_id).await;
    send_health(&router, node_id, 10.0).await;

    let storage = llm_router::db::models::ModelStorage::new(state.db_pool.clone());
    let model = llm_router::registry::models::ModelInfo::new(
        "model-registered".to_string(),
        0,
        "test".to_string(),
        0,
        vec![],
    );
    storage.save_model(&model).await.expect("save model");

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-registered", "hi"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = read_json(response).await;
    assert_eq!(body["error"]["code"], "no_capable_nodes");
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: /v0/nodes, /v0/healthエンドポイントが削除されたため (SPEC-66555000)"]
async fn test_request_for_unknown_model_returns_404() {
    let (_state, router, _guard) = build_test_router().await;

    let node = MockServer::start().await;
    mount_models(&node, &["model-a"]).await;

    let node_id = register_node(&router, &node, "node-a").await;
    approve_node(&router, node_id).await;
    send_health(&router, node_id, 10.0).await;

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-unknown", "hi"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await;
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: state.registryへのアクセスが削除されたため (SPEC-66555000)"]
async fn test_excludes_model_after_inference_failure() {
    let (state, router, _guard) = build_test_router().await;

    let node_fail = MockServer::start().await;
    let node_ok = MockServer::start().await;

    mount_models(&node_fail, &["model-x"]).await;
    mount_models(&node_ok, &["model-x"]).await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"messages": [{"content": "fail"}]})))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&node_fail)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"messages": [{"content": "ok"}]})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "chat.completion",
                "choices": [{
                    "message": {"role": "assistant", "content": "from-ok"},
                    "finish_reason": "stop",
                    "index": 0
                }]
            })),
        )
        .mount(&node_ok)
        .await;

    let fail_id = register_node(&router, &node_fail, "node-fail").await;
    let ok_id = register_node(&router, &node_ok, "node-ok").await;
    approve_node(&router, fail_id).await;
    approve_node(&router, ok_id).await;
    // Force selection to node_fail first
    send_health(&router, fail_id, 10.0).await;
    send_health(&router, ok_id, 90.0).await;

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-x", "fail"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let node = state.registry.get(fail_id).await.expect("get node");
    assert!(node.excluded_models.contains("model-x"));

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-x", "ok"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["choices"][0]["message"]["content"], "from-ok");
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: state.registryへのアクセスが削除されたため (SPEC-66555000)"]
async fn test_model_restored_after_node_reregistration() {
    let (state, router, _guard) = build_test_router().await;

    let node_fail = MockServer::start().await;
    let node_ok = MockServer::start().await;

    mount_models(&node_fail, &["model-x"]).await;
    mount_models(&node_ok, &["model-x"]).await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"messages": [{"content": "fail"}]})))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&node_fail)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"messages": [{"content": "restore"}]})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "chat.completion",
                "choices": [{
                    "message": {"role": "assistant", "content": "restored"},
                    "finish_reason": "stop",
                    "index": 0
                }]
            })),
        )
        .mount(&node_fail)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"messages": [{"content": "ok"}]})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "chat.completion",
                "choices": [{
                    "message": {"role": "assistant", "content": "from-ok"},
                    "finish_reason": "stop",
                    "index": 0
                }]
            })),
        )
        .mount(&node_ok)
        .await;

    let fail_id = register_node(&router, &node_fail, "node-fail").await;
    let ok_id = register_node(&router, &node_ok, "node-ok").await;
    approve_node(&router, fail_id).await;
    approve_node(&router, ok_id).await;
    send_health(&router, fail_id, 10.0).await;
    send_health(&router, ok_id, 90.0).await;

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-x", "fail"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let node = state.registry.get(fail_id).await.expect("get node");
    assert!(node.excluded_models.contains("model-x"));

    // Re-register node (same machine_name/runtime_port) to clear exclusion
    let updated_id = register_node(&router, &node_fail, "node-fail").await;
    assert_eq!(updated_id, fail_id);
    approve_node(&router, updated_id).await;
    send_health(&router, updated_id, 5.0).await;

    let node = state.registry.get(fail_id).await.expect("get node");
    assert!(node.excluded_models.is_empty());

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("model-x", "restore"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["choices"][0]["message"]["content"], "restored");
}

#[tokio::test]
#[serial]
#[ignore = "NodeRegistry廃止: /v0/nodes, /v0/healthエンドポイントが削除されたため (SPEC-66555000)"]
async fn test_metal_model_not_routed_to_cuda_node() {
    let (_state, router, _guard) = build_test_router().await;

    let metal_node = MockServer::start().await;
    let cuda_node = MockServer::start().await;

    mount_models(&metal_node, &["gpt-oss-metal-only"]).await;
    mount_models(&cuda_node, &["gpt-oss-cuda-only"]).await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({
                "object": "chat.completion",
                "choices": [{
                    "message": {"role": "assistant", "content": "from-metal"},
                    "finish_reason": "stop",
                    "index": 0
                }]
            })),
        )
        .mount(&metal_node)
        .await;

    let metal_id = register_node(&router, &metal_node, "node-metal").await;
    let cuda_id = register_node(&router, &cuda_node, "node-cuda").await;
    approve_node(&router, metal_id).await;
    approve_node(&router, cuda_id).await;
    send_health(&router, metal_id, 10.0).await;
    send_health(&router, cuda_id, 10.0).await;

    let response = post_json(
        &router,
        "/v1/chat/completions",
        chat_payload("gpt-oss-metal-only", "hi"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["choices"][0]["message"]["content"], "from-metal");
}
