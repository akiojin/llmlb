//! Issue #950: HTTP contracts for missing-only global text-model resolution.

use std::{sync::Arc, time::Duration};

use llmlb::{
    common::auth::UserRole,
    db::{endpoints, settings::SettingsStorage},
    types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI},
};
use reqwest::{Client, Response, StatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, Request, Respond, ResponseTemplate,
};

#[path = "support/mod.rs"]
mod support;

use support::{
    http::{spawn_lb, TestServer},
    lb::{build_test_router, create_test_db_pool, test_jwt_secret},
};

const MODEL_A: &str = "default-contract-a";
const MODEL_B: &str = "default-contract-b";
const ROUTES: [&str; 5] = [
    "/v1/chat/completions",
    "/v1/completions",
    "/v1/responses",
    "/v1/messages",
    "/api/dashboard/playground/chat/completions",
];

#[derive(Clone)]
struct EchoResponse;

impl Respond for EchoResponse {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = request.body_json().expect("upstream JSON");
        ResponseTemplate::new(200).set_body_json(json!({
            "id": "contract-response",
            "object": "chat.completion",
            "model": body["model"],
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "contract answer"},
                "finish_reason": "stop"
            }],
            "response": "contract answer",
            "output": [{"type": "message", "role": "assistant", "content": [
                {"type": "output_text", "text": "contract answer"}
            ]}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 2, "total_tokens": 5}
        }))
    }
}

struct Fixture {
    lb: TestServer,
    pool: SqlitePool,
    upstream: MockServer,
    prefix: String,
    client: Client,
    jwt: String,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_apis(vec![
            SupportedAPI::ChatCompletions,
            SupportedAPI::Responses,
            SupportedAPI::Embeddings,
            SupportedAPI::ImageInput,
        ])
        .await
    }

    async fn with_apis(apis: Vec<SupportedAPI>) -> Self {
        Self::with_models(apis, &[(MODEL_A, None), (MODEL_B, None)]).await
    }

    async fn with_models(apis: Vec<SupportedAPI>, models: &[(&str, Option<&str>)]) -> Self {
        let pool = create_test_db_pool().await;
        let upstream = MockServer::start().await;
        let prefix = format!("/{}", Uuid::new_v4());
        for route in ["chat/completions", "completions", "responses", "embeddings"] {
            Mock::given(method("POST"))
                .and(path(format!("{prefix}/v1/{route}")))
                .respond_with(EchoResponse)
                .mount(&upstream)
                .await;
        }
        seed_endpoint(&pool, &format!("{}{prefix}", upstream.uri()), models, apis).await;
        let user = llmlb::db::users::create(
            &pool,
            "inference-contract-admin",
            "unused-password-hash",
            UserRole::Admin,
            false,
        )
        .await
        .unwrap();
        let jwt = llmlb::auth::jwt::create_jwt(
            &user.id.to_string(),
            UserRole::Admin,
            &test_jwt_secret(),
            false,
            user.password_changed_at,
        )
        .unwrap();
        let lb = spawn_lb(build_test_router(pool.clone()).await).await;
        Self {
            lb,
            pool,
            upstream,
            prefix,
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            jwt,
        }
    }

    async fn set_raw(&self, raw: &str) {
        SettingsStorage::new(self.pool.clone())
            .set_setting("inference.defaults", raw)
            .await
            .unwrap();
    }

    async fn set_default(&self, model: Option<&str>) {
        self.set_raw(&json!({"version": 1, "default_text_model": model}).to_string())
            .await;
    }

    async fn post(&self, route: &str, payload: &Value) -> Response {
        self.client
            .post(format!("http://{}{route}", self.lb.addr()))
            .bearer_auth(if route.starts_with("/api/") {
                &self.jwt
            } else {
                "sk_debug"
            })
            .header("anthropic-version", "2023-06-01")
            .json(payload)
            .send()
            .await
            .unwrap()
    }

    async fn requests(&self) -> Vec<Request> {
        self.upstream.received_requests().await.unwrap()
    }
}

async fn seed_endpoint(
    pool: &SqlitePool,
    base_url: &str,
    models: &[(&str, Option<&str>)],
    supported_apis: Vec<SupportedAPI>,
) -> Uuid {
    let mut endpoint = Endpoint::new(
        format!("inference-contract-{}", Uuid::new_v4()),
        base_url.to_string(),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    endpoints::create_endpoint(pool, &endpoint).await.unwrap();
    for (model, canonical) in models {
        endpoints::add_endpoint_model(
            pool,
            &EndpointModel {
                endpoint_id: endpoint.id,
                model_id: (*model).to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: supported_apis.clone(),
                canonical_name: canonical.map(str::to_string),
            },
        )
        .await
        .unwrap();
    }
    endpoint.id
}

fn payload_for(route: &str) -> Value {
    match route {
        "/v1/completions" => json!({"prompt": "contract prompt", "max_tokens": 8}),
        "/v1/responses" => json!({"input": "contract prompt", "max_output_tokens": 8}),
        _ => json!({
            "messages": [{"role": "user", "content": "contract prompt"}],
            "max_tokens": 8
        }),
    }
}

#[tokio::test]
async fn missing_model_uses_default_on_all_text_ingresses_and_preserves_explicit_model() {
    let fixture = Fixture::new().await;
    fixture.set_default(Some(MODEL_A)).await;
    for route in ROUTES {
        let response = fixture.post(route, &payload_for(route)).await;
        let status = response.status();
        let body = response.text().await.unwrap();
        assert_eq!(status, StatusCode::OK, "missing model at {route}: {body}");
        let requests = fixture.requests().await;
        let upstream: Value = requests.last().unwrap().body_json().unwrap();
        assert_eq!(upstream["model"], MODEL_A, "default at {route}");
        let mut explicit = payload_for(route);
        explicit["model"] = json!(MODEL_B);
        let response = fixture.post(route, &explicit).await;
        assert_eq!(response.status(), StatusCode::OK, "explicit at {route}");
        let requests = fixture.requests().await;
        let upstream: Value = requests.last().unwrap().body_json().unwrap();
        assert_eq!(upstream["model"], MODEL_B, "explicit at {route}");
    }
    assert_eq!(
        fixture.requests().await.len(),
        10,
        "one upstream dispatch per request"
    );
    fixture.lb.stop().await;
}

#[tokio::test]
async fn explicit_invalid_and_nonobject_inputs_keep_their_unconfigured_errors() {
    let fixture = Fixture::new().await;
    for route in ROUTES {
        for model in [
            Value::Null,
            json!(7),
            json!({}),
            json!([]),
            json!(""),
            json!(" "),
        ] {
            let mut payload = payload_for(route);
            payload["model"] = model.clone();
            fixture.set_default(None).await;
            let baseline = fixture.post(route, &payload).await;
            let baseline_status = baseline.status();
            let baseline_body = baseline.text().await.unwrap();
            assert!(
                baseline_status.is_client_error(),
                "baseline {route}/{model}"
            );
            fixture.set_default(Some(MODEL_A)).await;
            let configured = fixture.post(route, &payload).await;
            let configured_status = configured.status();
            let configured_body = configured.text().await.unwrap();
            assert_eq!(configured_status, baseline_status, "{route}/{model}");
            assert_eq!(configured_body, baseline_body, "{route}/{model}");
        }
        for payload in [Value::Null, json!([]), json!("input"), json!(42)] {
            fixture.set_default(None).await;
            let baseline = fixture.post(route, &payload).await;
            let baseline_status = baseline.status();
            let baseline_body = baseline.text().await.unwrap();
            fixture.set_default(Some(MODEL_A)).await;
            let configured = fixture.post(route, &payload).await;
            assert_eq!(
                configured.status(),
                baseline_status,
                "nonobject {route}/{payload}"
            );
            assert_eq!(
                configured.text().await.unwrap(),
                baseline_body,
                "{route}/{payload}"
            );
        }
    }
    assert!(
        fixture.requests().await.is_empty(),
        "invalid values never dispatch"
    );
    fixture.lb.stop().await;
}

#[tokio::test]
async fn unset_default_keeps_missing_model_errors_and_explicit_failure_never_falls_back() {
    let fixture = Fixture::new().await;
    for default in [None, Some(MODEL_A)] {
        fixture.set_default(default).await;
        for route in ROUTES {
            if default.is_none() {
                assert_eq!(
                    fixture.post(route, &payload_for(route)).await.status(),
                    StatusCode::BAD_REQUEST
                );
            }
            let mut explicit = payload_for(route);
            explicit["model"] = json!("nonexistent-contract-model");
            assert_eq!(
                fixture.post(route, &explicit).await.status(),
                StatusCode::NOT_FOUND
            );
        }
    }
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}

#[tokio::test]
async fn corrupt_defaults_fail_closed_only_when_the_model_key_is_missing() {
    let fixture = Fixture::new().await;
    for raw in [
        "not-json",
        r#"{"version":99,"default_text_model":"default-contract-a"}"#,
        r#"{"version":1,"default_text_model":false}"#,
        r#"{"version":1,"default_text_model":""}"#,
    ] {
        fixture.set_raw(raw).await;
        for route in ROUTES {
            let missing = fixture.post(route, &payload_for(route)).await;
            assert!(
                missing.status().is_server_error(),
                "{route} must fail closed for {raw}"
            );
            let mut explicit = payload_for(route);
            explicit["model"] = json!(MODEL_B);
            assert_eq!(
                fixture.post(route, &explicit).await.status(),
                StatusCode::OK,
                "explicit {route} bypasses corrupt config"
            );
        }
    }
    fixture.lb.stop().await;
}

#[tokio::test]
async fn explicit_requests_do_not_depend_on_the_settings_table() {
    let fixture = Fixture::new().await;
    sqlx::query("DROP TABLE settings")
        .execute(&fixture.pool)
        .await
        .unwrap();
    for route in ROUTES {
        let mut payload = payload_for(route);
        payload["model"] = json!(MODEL_B);
        assert_eq!(
            fixture.post(route, &payload).await.status(),
            StatusCode::OK,
            "explicit {route} must not read settings"
        );
        payload["model"] = Value::Null;
        assert_eq!(
            fixture.post(route, &payload).await.status(),
            StatusCode::BAD_REQUEST,
            "explicit null {route} must not read settings"
        );
        assert_eq!(
            fixture.post(route, &json!([])).await.status(),
            StatusCode::BAD_REQUEST,
            "nonobject {route} must not read settings"
        );
        assert!(
            fixture
                .post(route, &payload_for(route))
                .await
                .status()
                .is_server_error(),
            "missing {route} must report settings read failure"
        );
    }
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_cloud_models_reject_routes_without_existing_adapters() {
    let fixture = Fixture::new().await;
    for (model, routes) in [
        (
            "openai:gpt-4o",
            vec!["/v1/completions", "/v1/responses", "/v1/messages"],
        ),
        (
            "google:gemini-pro",
            vec!["/v1/completions", "/v1/responses", "/v1/messages"],
        ),
        (
            "anthropic:claude-test",
            vec!["/v1/completions", "/v1/responses"],
        ),
    ] {
        fixture.set_default(Some(model)).await;
        for route in routes {
            let response = fixture.post(route, &payload_for(route)).await;
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{model} at {route}"
            );
            let body: Value = response.json().await.unwrap();
            assert!(body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("configured default model does not support this API"));
            if route == "/v1/messages" {
                assert_eq!(body["error"]["type"], "invalid_request_error");
            }
        }
    }
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_route_capabilities_are_required_without_changing_explicit_requests() {
    let fixture = Fixture::with_apis(vec![SupportedAPI::Responses]).await;
    fixture.set_default(Some(MODEL_A)).await;
    for route in ["/v1/chat/completions", "/v1/messages"] {
        let response = fixture.post(route, &payload_for(route)).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "default {route}"
        );
        let mut explicit = payload_for(route);
        explicit["model"] = json!(MODEL_A);
        assert_eq!(
            fixture.post(route, &explicit).await.status(),
            StatusCode::OK,
            "existing explicit {route}"
        );
    }
    assert_eq!(
        fixture
            .post("/v1/responses", &payload_for("/v1/responses"))
            .await
            .status(),
        StatusCode::OK
    );
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_image_input_requires_route_and_image_support_on_the_same_endpoint() {
    let pool = create_test_db_pool().await;
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    Mock::given(method("POST"))
        .and(path(format!("{prefix}/v1/chat/completions")))
        .respond_with(EchoResponse)
        .mount(&upstream)
        .await;
    // Neither endpoint satisfies both requirements, although the union does.
    for (label, apis) in [
        ("chat-only", vec![SupportedAPI::ChatCompletions]),
        ("image-only", vec![SupportedAPI::ImageInput]),
    ] {
        seed_endpoint(
            &pool,
            &format!("{}{prefix}/{label}", upstream.uri()),
            &[(MODEL_A, None)],
            apis,
        )
        .await;
    }
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool).await).await;
    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .bearer_auth("sk_debug")
        .json(&json!({"messages":[{"role":"user","content":[
            {"type":"text","text":"describe"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,aGVsbG8="}}
        ]}]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        upstream.received_requests().await.unwrap().is_empty(),
        "must not dispatch to either partial candidate"
    );
    lb.stop().await;
}

#[tokio::test]
async fn global_default_does_not_replace_embedding_or_media_model_contracts() {
    let fixture = Fixture::new().await;
    fixture.set_default(Some(MODEL_A)).await;
    let response = fixture
        .post("/v1/embeddings", &json!({"input":"embedding input"}))
        .await;
    assert_ne!(
        response.status(),
        StatusCode::OK,
        "embedding must not silently use text default"
    );
    for (route, payload) in [
        ("/v1/images/generations", json!({"prompt":"image"})),
        (
            "/v1/audio/speech",
            json!({"input":"voice", "voice":"alloy"}),
        ),
    ] {
        let response = fixture.post(route, &payload).await;
        assert!(
            response.status().is_client_error(),
            "{route} keeps model mandatory"
        );
    }
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_messages_keeps_required_header_and_max_tokens_validation() {
    let fixture = Fixture::new().await;
    fixture.set_default(Some(MODEL_A)).await;
    let response = fixture
        .client
        .post(format!("http://{}/v1/messages", fixture.lb.addr()))
        .bearer_auth("sk_debug")
        .json(&payload_for("/v1/messages"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "anthropic-version remains mandatory"
    );
    let mut payload = payload_for("/v1/messages");
    payload.as_object_mut().unwrap().remove("max_tokens");
    assert_eq!(
        fixture.post("/v1/messages", &payload).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_alias_dispatch_and_history_match_the_same_explicit_public_model() {
    let backend_model = "endpoint-contract-alias-a";
    let fixture = Fixture::with_models(
        vec![SupportedAPI::ChatCompletions, SupportedAPI::Responses],
        &[(backend_model, Some(MODEL_A))],
    )
    .await;
    fixture.set_default(Some(MODEL_A)).await;
    for route in ROUTES {
        for explicit in [false, true] {
            let mut payload = payload_for(route);
            if explicit {
                payload["model"] = json!(MODEL_A);
            }
            let response = fixture.post(route, &payload).await;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{route}, explicit={explicit}"
            );
            let requests = fixture.requests().await;
            let received: Value = requests.last().unwrap().body_json().unwrap();
            assert_eq!(
                received["model"], backend_model,
                "same alias resolution at {route}"
            );
        }
    }
    let history = llmlb::db::request_history::RequestHistoryStorage::new(fixture.pool.clone());
    let records = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let records = history.load_records().await.unwrap();
            if records.len() >= 8 {
                break records;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("history writes complete");
    // Responses intentionally has no detailed request-history writer.
    assert_eq!(records.len(), 8, "existing history records exactly once");
    for record in records {
        assert_eq!(record.model, MODEL_A);
        assert_eq!(
            record.request_body["model"], MODEL_A,
            "snapshot precedes history capture"
        );
    }
    assert_eq!(fixture.requests().await.len(), 10);
    assert_model_stats(&fixture.pool, &[(MODEL_A, 10)]).await;
    fixture.lb.stop().await;
}

#[derive(Clone)]
struct StreamingBarrier {
    accepted: tokio::sync::mpsc::UnboundedSender<String>,
    release: Arc<tokio::sync::Notify>,
}

async fn barrier_chat(
    axum::extract::State(state): axum::extract::State<StreamingBarrier>,
    axum::Json(payload): axum::Json<Value>,
) -> axum::response::Response {
    use axum::{body::Body, response::IntoResponse};
    state
        .accepted
        .send(payload["model"].as_str().unwrap().to_string())
        .unwrap();
    if payload["stream"].as_bool() != Some(true) {
        return axum::Json(json!({
            "id":"barrier-nonstream", "model":payload["model"],
            "choices":[{"index":0,"message":{"role":"assistant","content":"B answer"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}
        })).into_response();
    }
    let stream = futures::stream::unfold((0_u8, state.release), |(phase, release)| async move {
        let chunk = match phase {
            0 => "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"A answer\"}}]}\n\n",
            1 => {
                release.notified().await;
                "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\ndata: [DONE]\n\n"
            }
            _ => return None,
        };
        Some((Ok::<_, std::io::Error>(chunk), (phase + 1, release)))
    });
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap()
}

#[tokio::test]
async fn changing_default_during_stream_keeps_accepted_model_and_updates_the_next_request() {
    let pool = create_test_db_pool().await;
    let (accepted, mut received) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let prefix = format!("/{}", Uuid::new_v4());
    let upstream = spawn_lb(
        axum::Router::new()
            .route(
                &format!("{prefix}/v1/chat/completions"),
                axum::routing::post(barrier_chat),
            )
            .with_state(StreamingBarrier {
                accepted,
                release: release.clone(),
            }),
    )
    .await;
    seed_endpoint(
        &pool,
        &format!("http://{}{prefix}", upstream.addr()),
        &[(MODEL_A, None), (MODEL_B, None)],
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
    let settings = SettingsStorage::new(pool.clone());
    settings
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool.clone()).await).await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let mut streaming_payload = payload_for("/v1/chat/completions");
    streaming_payload["stream"] = json!(true);
    let mut response = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .bearer_auth("sk_debug")
        .json(&streaming_payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(received.recv().await.as_deref(), Some(MODEL_A));
    let first = response.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("A answer"));
    settings
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_B}).to_string(),
        )
        .await
        .unwrap();
    let next = client
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .bearer_auth("sk_debug")
        .json(&payload_for("/v1/chat/completions"))
        .send()
        .await
        .unwrap();
    assert_eq!(next.status(), StatusCode::OK);
    assert_eq!(received.recv().await.as_deref(), Some(MODEL_B));
    release.notify_one();
    assert!(response.text().await.unwrap().contains("[DONE]"));
    let history = llmlb::db::request_history::RequestHistoryStorage::new(pool.clone());
    let records = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let records = history.load_records().await.unwrap();
            if records.len() >= 2 {
                break records;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stream and next-request history writes");
    assert_eq!(records.len(), 2, "one history record per accepted request");
    for model in [MODEL_A, MODEL_B] {
        let records: Vec<_> = records
            .iter()
            .filter(|record| record.model == model)
            .collect();
        assert_eq!(records.len(), 1, "model attribution stays fixed");
        assert_eq!(records[0].request_body["model"], model);
    }
    assert_model_stats(&pool, &[(MODEL_A, 1), (MODEL_B, 1)]).await;
    lb.stop().await;
    upstream.stop().await;
}

async fn assert_model_stats(pool: &SqlitePool, expected: &[(&str, i64)]) {
    let expected_total: i64 = expected.iter().map(|(_, count)| count).sum();
    let stats: Vec<(String, i64)> =
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let stats: Vec<(String, i64)> = sqlx::query_as(
                "SELECT model_id, SUM(total_requests) FROM endpoint_daily_stats GROUP BY model_id",
            ).fetch_all(pool).await.unwrap();
                if stats.iter().map(|(_, count)| count).sum::<i64>() >= expected_total {
                    break stats;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("endpoint model-stat writes complete");
    assert_eq!(stats.len(), expected.len(), "no extra model attribution");
    for (model, count) in expected {
        assert_eq!(
            stats.iter().find(|(id, _)| id == model).map(|(_, n)| *n),
            Some(*count),
            "endpoint stats exactly once for {model}"
        );
    }
}

fn image_payload_for(route: &str) -> Value {
    let image_url = "data:image/png;base64,aGVsbG8=";
    match route {
        "/v1/responses" => json!({"input":[{"role":"user","content":[
            {"type":"input_text","text":"describe"},
            {"type":"input_image","image_url":image_url}
        ]}]}),
        "/v1/messages" => json!({"max_tokens":8,"messages":[{"role":"user","content":[
            {"type":"text","text":"describe"},
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"aGVsbG8="}}
        ]}]}),
        _ => json!({"messages":[{"role":"user","content":[
            {"type":"text","text":"describe"},
            {"type":"image_url","image_url":{"url":image_url}}
        ]}]}),
    }
}

#[tokio::test]
async fn default_image_requests_select_only_the_candidate_with_both_required_apis() {
    let pool = create_test_db_pool().await;
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    for (label, apis) in [
        ("chat-only", vec![SupportedAPI::ChatCompletions]),
        ("responses-only", vec![SupportedAPI::Responses]),
        ("image-only", vec![SupportedAPI::ImageInput]),
        (
            "capable",
            vec![
                SupportedAPI::ChatCompletions,
                SupportedAPI::Responses,
                SupportedAPI::ImageInput,
            ],
        ),
    ] {
        seed_endpoint(
            &pool,
            &format!("{}{prefix}/{label}", upstream.uri()),
            &[(MODEL_A, None)],
            apis,
        )
        .await;
        for route in ["chat/completions", "responses"] {
            Mock::given(method("POST"))
                .and(path(format!("{prefix}/{label}/v1/{route}")))
                .respond_with(EchoResponse)
                .mount(&upstream)
                .await;
        }
    }
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool).await).await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    for route in ["/v1/chat/completions", "/v1/responses"] {
        for _ in 0..4 {
            let response = client
                .post(format!("http://{}{route}", lb.addr()))
                .bearer_auth("sk_debug")
                .json(&image_payload_for(route))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{route}");
        }
    }
    let requests = upstream.received_requests().await.unwrap();
    assert_eq!(requests.len(), 8);
    for request in requests {
        assert!(
            request
                .url
                .path()
                .starts_with(&format!("{prefix}/capable/")),
            "{} must satisfy route AND image support",
            request.url
        );
        let body: Value = request.body_json().unwrap();
        assert_eq!(body["model"], MODEL_A);
    }
    lb.stop().await;
}

#[tokio::test]
async fn different_runtime_alias_rows_cannot_combine_their_required_api_capabilities() {
    let pool = create_test_db_pool().await;
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    let id = seed_endpoint(
        &pool,
        &format!("{}{prefix}", upstream.uri()),
        &[("chat-alias", Some(MODEL_A))],
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
    endpoints::add_endpoint_model(
        &pool,
        &EndpointModel {
            endpoint_id: id,
            model_id: "image-alias".into(),
            canonical_name: Some(MODEL_A.into()),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ImageInput],
        },
    )
    .await
    .unwrap();
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool).await).await;
    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .bearer_auth("sk_debug")
        .json(&image_payload_for("/v1/chat/completions"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "the dispatched model row itself must satisfy both APIs"
    );
    assert!(upstream.received_requests().await.unwrap().is_empty());
    lb.stop().await;
}

#[tokio::test]
async fn default_image_cannot_borrow_capabilities_from_an_alias_the_rewriter_will_not_dispatch() {
    let pool = create_test_db_pool().await;
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    Mock::given(method("POST"))
        .and(path(format!("{prefix}/v1/chat/completions")))
        .respond_with(EchoResponse)
        .mount(&upstream)
        .await;
    let endpoint_id = seed_endpoint(
        &pool,
        &format!("{}{prefix}", upstream.uri()),
        &[("a-unsupported", Some(MODEL_A))],
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
    endpoints::add_endpoint_model(
        &pool,
        &EndpointModel {
            endpoint_id,
            model_id: "b-supported".into(),
            canonical_name: Some(MODEL_A.into()),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions, SupportedAPI::ImageInput],
        },
    )
    .await
    .unwrap();
    let rows = endpoints::list_endpoint_models(&pool, endpoint_id)
        .await
        .unwrap();
    assert_eq!(
        rows[0].model_id, "a-unsupported",
        "fixture fixes the existing first-match alias order"
    );
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool).await).await;
    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", lb.addr()))
        .bearer_auth("sk_debug")
        .json(&image_payload_for("/v1/chat/completions"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "the alias chosen by the existing rewriter must itself support image input"
    );
    assert!(
        upstream.received_requests().await.unwrap().is_empty(),
        "b-supported must not lend image capability to dispatched a-unsupported"
    );
    lb.stop().await;
}

#[tokio::test]
async fn known_text_only_and_dedicated_models_reject_incompatible_default_requests() {
    use llmlb::{
        db::models::ModelStorage, registry::models::ModelInfo, types::model::ModelCapability,
    };
    let fixture = Fixture::new().await;
    let storage = ModelStorage::new(fixture.pool.clone());
    for capability in [
        ModelCapability::Embedding,
        ModelCapability::ImageGeneration,
        ModelCapability::SpeechToText,
        ModelCapability::TextToSpeech,
    ] {
        storage
            .save_model(&ModelInfo::with_capabilities(
                MODEL_A.into(),
                1,
                "dedicated model".into(),
                1,
                vec![],
                vec![capability],
            ))
            .await
            .unwrap();
        fixture.set_default(Some(MODEL_A)).await;
        for route in ROUTES {
            assert_eq!(
                fixture.post(route, &payload_for(route)).await.status(),
                StatusCode::BAD_REQUEST,
                "{capability:?} at {route}"
            );
        }
    }
    storage
        .save_model(&ModelInfo::with_capabilities(
            MODEL_A.into(),
            1,
            "text only".into(),
            1,
            vec![],
            vec![ModelCapability::TextGeneration],
        ))
        .await
        .unwrap();
    for route in ["/v1/chat/completions", "/v1/responses"] {
        assert_eq!(
            fixture
                .post(route, &image_payload_for(route))
                .await
                .status(),
            StatusCode::BAD_REQUEST,
            "known text-only {route}"
        );
    }
    // The existing local Messages translator does not accept image blocks.
    for explicit in [false, true] {
        let mut payload = image_payload_for("/v1/messages");
        if explicit {
            payload["model"] = json!(MODEL_A);
        }
        assert_eq!(
            fixture.post("/v1/messages", &payload).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_nonexistent_and_known_offline_models_do_not_fall_back() {
    let fixture = Fixture::new().await;
    fixture
        .set_default(Some("nonexistent-default-contract"))
        .await;
    for route in ROUTES {
        assert_eq!(
            fixture.post(route, &payload_for(route)).await.status(),
            StatusCode::NOT_FOUND,
            "missing default {route}"
        );
    }
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
    let pool = create_test_db_pool().await;
    let upstream = MockServer::start().await;
    let id = seed_endpoint(
        &pool,
        &format!("{}/{}", upstream.uri(), Uuid::new_v4()),
        &[(MODEL_A, None)],
        vec![SupportedAPI::ChatCompletions, SupportedAPI::Responses],
    )
    .await;
    sqlx::query("UPDATE endpoints SET status='offline' WHERE id=?")
        .bind(id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool).await).await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    for route in &ROUTES[..4] {
        let response = client
            .post(format!("http://{}{route}", lb.addr()))
            .bearer_auth("sk_debug")
            .header("anthropic-version", "2023-06-01")
            .json(&payload_for(route))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "known offline default {route}"
        );
    }
    assert!(upstream.received_requests().await.unwrap().is_empty());
    lb.stop().await;
}

#[tokio::test]
async fn failed_alias_dispatch_keeps_public_history_and_records_stats_once() {
    let fixture = Fixture::with_models(
        vec![SupportedAPI::ChatCompletions, SupportedAPI::Responses],
        &[("failure-backend-alias", Some(MODEL_A))],
    )
    .await;
    for route in ["chat/completions", "completions", "responses"] {
        Mock::given(method("POST"))
            .and(path(format!("{}/v1/{route}", fixture.prefix)))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_json(json!({"error":{"message":"contract upstream unavailable"}})),
            )
            .with_priority(1)
            .mount(&fixture.upstream)
            .await;
    }
    fixture.set_default(Some(MODEL_A)).await;
    for route in ROUTES {
        for explicit in [false, true] {
            let mut payload = payload_for(route);
            if explicit {
                payload["model"] = json!(MODEL_A);
            }
            let response = fixture.post(route, &payload).await;
            assert!(
                response.status().is_server_error(),
                "{route}, explicit={explicit}"
            );
        }
    }
    let requests = fixture.requests().await;
    assert_eq!(
        requests.len(),
        10,
        "upstream failures are never retried with another model"
    );
    for request in requests {
        let body: Value = request.body_json().unwrap();
        assert_eq!(body["model"], "failure-backend-alias");
    }
    assert_model_stats(&fixture.pool, &[(MODEL_A, 10)]).await;
    let failures: i64 = sqlx::query_scalar("SELECT SUM(failed_requests) FROM endpoint_daily_stats")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(failures, 10);
    let history = llmlb::db::request_history::RequestHistoryStorage::new(fixture.pool.clone());
    let records = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let records = history.load_records().await.unwrap();
            if records.len() >= 8 {
                break records;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("failed request history writes");
    assert_eq!(
        records.len(),
        8,
        "Responses keeps existing no-detail-history behavior"
    );
    for record in records {
        assert_eq!(record.model, MODEL_A);
        assert_eq!(record.request_body["model"], MODEL_A);
        assert!(matches!(
            record.status,
            llmlb::common::protocol::RecordStatus::Error { .. }
        ));
    }
    fixture.lb.stop().await;
}

#[tokio::test]
async fn default_and_explicit_stream_disconnects_attribute_one_terminal_stat_each() {
    let pool = create_test_db_pool().await;
    let (accepted, mut received) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let prefix = format!("/{}", Uuid::new_v4());
    let upstream = spawn_lb(
        axum::Router::new()
            .route(
                &format!("{prefix}/v1/chat/completions"),
                axum::routing::post(barrier_chat),
            )
            .with_state(StreamingBarrier {
                accepted,
                release: release.clone(),
            }),
    )
    .await;
    seed_endpoint(
        &pool,
        &format!("http://{}{prefix}", upstream.addr()),
        &[(MODEL_A, None)],
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL_A}).to_string(),
        )
        .await
        .unwrap();
    let lb = spawn_lb(build_test_router(pool.clone()).await).await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    for (index, explicit) in [false, true].into_iter().enumerate() {
        let mut payload = payload_for("/v1/chat/completions");
        payload["stream"] = json!(true);
        if explicit {
            payload["model"] = json!(MODEL_A);
        }
        let mut response = client
            .post(format!("http://{}/v1/chat/completions", lb.addr()))
            .bearer_auth("sk_debug")
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(received.recv().await.as_deref(), Some(MODEL_A));
        assert!(response.chunk().await.unwrap().is_some());
        // Cancel after a real first chunk while the upstream is still blocked.
        drop(response);
        assert_model_stats(&pool, &[(MODEL_A, (index + 1) as i64)]).await;
    }
    let history = llmlb::db::request_history::RequestHistoryStorage::new(pool);
    let records = history.load_records().await.unwrap();
    assert_eq!(
        records.len(),
        2,
        "disconnect must not add a second history writer"
    );
    for record in records {
        assert_eq!(record.model, MODEL_A);
        assert_eq!(record.request_body["model"], MODEL_A);
    }
    release.notify_waiters();
    lb.stop().await;
    upstream.stop().await;
}

#[tokio::test]
#[serial_test::serial]
async fn default_cloud_requests_respect_known_modality_and_text_only_unknown_metadata() {
    use llmlb::{
        db::models::ModelStorage, registry::models::ModelInfo, types::model::ModelCapability,
    };
    struct RestoreEnv(&'static str, Option<std::ffi::OsString>);
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            if let Some(value) = self.1.as_ref() {
                std::env::set_var(self.0, value);
            } else {
                std::env::remove_var(self.0);
            }
        }
    }
    let _key = RestoreEnv("OPENAI_API_KEY", std::env::var_os("OPENAI_API_KEY"));
    let _base = RestoreEnv("OPENAI_BASE_URL", std::env::var_os("OPENAI_BASE_URL"));
    let cloud = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    Mock::given(method("POST"))
        .and(path(format!("{prefix}/v1/chat/completions")))
        .respond_with(EchoResponse)
        .mount(&cloud)
        .await;
    // Any erroneous cloud dispatch remains on this isolated test server.
    std::env::set_var("OPENAI_API_KEY", "test-only-inference-cloud-key");
    std::env::set_var("OPENAI_BASE_URL", format!("{}{prefix}", cloud.uri()));
    let fixture = Fixture::new().await;
    let storage = ModelStorage::new(fixture.pool.clone());
    let known_model = "openai:cloud-known";
    fixture.set_default(Some(known_model)).await;
    for capability in [
        ModelCapability::Embedding,
        ModelCapability::SpeechToText,
        ModelCapability::TextToSpeech,
        ModelCapability::ImageGeneration,
    ] {
        storage
            .save_model(&ModelInfo::with_capabilities(
                known_model.into(),
                1,
                "cloud modality".into(),
                1,
                vec![],
                vec![capability],
            ))
            .await
            .unwrap();
        for route in ["/v1/chat/completions", "/v1/responses"] {
            assert_eq!(
                fixture.post(route, &payload_for(route)).await.status(),
                StatusCode::BAD_REQUEST,
                "known cloud {capability:?} is incompatible with {route}"
            );
        }
    }
    storage
        .save_model(&ModelInfo::with_capabilities(
            known_model.into(),
            1,
            "cloud text only".into(),
            1,
            vec![],
            vec![ModelCapability::TextGeneration],
        ))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .post(
                "/v1/chat/completions",
                &image_payload_for("/v1/chat/completions")
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST,
        "known text-only cloud model must not receive image input"
    );
    fixture
        .set_default(Some("openai:cloud-unknown-metadata"))
        .await;
    assert_eq!(
        fixture
            .post(
                "/v1/chat/completions",
                &image_payload_for("/v1/chat/completions")
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST,
        "absent cloud metadata uses the existing text-generation-only inference"
    );
    assert!(
        cloud.received_requests().await.unwrap().is_empty(),
        "incompatible defaults never contact cloud"
    );
    assert!(fixture.requests().await.is_empty());
    fixture.lb.stop().await;
}
