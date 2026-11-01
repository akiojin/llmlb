//! Issue #950: default-origin cloud dispatch matches the same explicit model.

use std::{ffi::OsString, time::Duration};

use llmlb::{
    common::protocol::{RecordStatus, RequestResponseRecord, RequestType},
    db::{models::ModelStorage, request_history::RequestHistoryStorage, settings::SettingsStorage},
    registry::models::ModelInfo,
};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[path = "support/mod.rs"]
mod support;

use support::{
    http::spawn_lb,
    lb::{build_test_router, create_test_db_pool},
};

struct RestoreEnv(Vec<(&'static str, Option<OsString>)>);

impl RestoreEnv {
    fn set(values: &[(&'static str, String)]) -> Self {
        let original = values
            .iter()
            .map(|(name, _)| (*name, std::env::var_os(name)))
            .collect();
        for (name, value) in values {
            std::env::set_var(name, value);
        }
        Self(original)
    }
}

impl Drop for RestoreEnv {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

async fn save_default(pool: &SqlitePool, public_model: &str) {
    // Seed existing catalog metadata; default dispatch must not fetch any catalog.
    ModelStorage::new(pool.clone())
        .save_model(&ModelInfo::new(
            public_model.into(),
            1,
            "cloud contract".into(),
            1,
            vec![],
        ))
        .await
        .unwrap();
    SettingsStorage::new(pool.clone())
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":public_model}).to_string(),
        )
        .await
        .unwrap();
}

async fn recorded_once(
    pool: &SqlitePool,
    provider: &str,
    public_model: &str,
    request_type: RequestType,
    expected_request_body: &Value,
) -> Vec<RequestResponseRecord> {
    let history = RequestHistoryStorage::new(pool.clone());
    let (records, metrics) = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let records = history.load_records().await.unwrap();
            let metrics: Vec<(String, i64)> =
                sqlx::query_as("SELECT provider,status_code FROM cloud_request_metrics")
                    .fetch_all(pool)
                    .await
                    .unwrap();
            if records.len() >= 2 && metrics.len() >= 2 {
                break (records, metrics);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("two history and cloud-stat writes complete");
    assert_eq!(records.len(), 2, "one existing history writer per request");
    assert_eq!(
        metrics.len(),
        2,
        "one existing cloud metrics sample per request"
    );
    for (actual_provider, status) in metrics {
        assert_eq!(actual_provider, provider);
        assert_eq!(status, 200);
    }
    for record in &records {
        assert_eq!(record.model, public_model);
        assert_eq!(record.request_type, request_type);
        assert_eq!(
            &record.request_body, expected_request_body,
            "capture occurs after default resolution and before prefix stripping"
        );
        assert!(matches!(record.status, RecordStatus::Success));
        assert_eq!(record.endpoint_name, format!("cloud:{provider}"));
    }
    let local_stats: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM endpoint_daily_stats")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        local_stats, 0,
        "cloud paths retain their existing cloud-metrics writer"
    );
    records
}

fn upstream_chat_response(provider: &str, upstream_model: &str) -> Value {
    match provider {
        "google" => json!({
            "candidates":[{"content":{"parts":[{"text":"cloud contract answer"}]}}],
            "usageMetadata":{"promptTokenCount":3,"candidatesTokenCount":2,"totalTokenCount":5}
        }),
        "anthropic" => json!({
            "id":"msg_cloud_contract","type":"message","role":"assistant","model":upstream_model,
            "content":[{"type":"text","text":"cloud contract answer"}],
            "stop_reason":"end_turn","stop_sequence":null,
            "usage":{"input_tokens":3,"output_tokens":2}
        }),
        _ => json!({
            "id":"chatcmpl_cloud_contract","object":"chat.completion","model":upstream_model,
            "choices":[{"index":0,"message":{"role":"assistant","content":"cloud contract answer"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":3,"completion_tokens":2,"total_tokens":5}
        }),
    }
}

async fn chat_adapter_contract(provider: &str) {
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    let upstream_model = format!("cloud-contract-{provider}");
    let public_model = format!("{provider}:{upstream_model}");
    let (key_env, base_env, upstream_path) = match provider {
        "google" => (
            "GOOGLE_API_KEY",
            "GOOGLE_API_BASE_URL",
            format!("{prefix}/models/{upstream_model}:generateContent"),
        ),
        "anthropic" => (
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_API_BASE_URL",
            format!("{prefix}/v1/messages"),
        ),
        _ => (
            "OPENAI_API_KEY",
            "OPENAI_BASE_URL",
            format!("{prefix}/v1/chat/completions"),
        ),
    };
    let fake_key = format!("test-only-{provider}-cloud-key");
    // Both authentication and base URL are controlled and restored, so no request
    // can use ambient credentials or a real provider endpoint.
    let _env = RestoreEnv::set(&[
        (key_env, fake_key.clone()),
        (base_env, format!("{}{prefix}", upstream.uri())),
    ]);
    Mock::given(method("POST"))
        .and(path(&upstream_path))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(upstream_chat_response(provider, &upstream_model)),
        )
        .mount(&upstream)
        .await;
    let pool = create_test_db_pool().await;
    save_default(&pool, &public_model).await;
    let lb = spawn_lb(build_test_router(pool.clone()).await).await;
    let client = client();
    let payload = json!({"messages":[{"role":"user","content":"cloud contract prompt"}],"max_tokens":8,"temperature":0.2});
    let mut explicit_payload = payload.clone();
    explicit_payload["model"] = json!(public_model);
    for request_body in [&payload, &explicit_payload] {
        let response = client
            .post(format!("http://{}/v1/chat/completions", lb.addr()))
            .bearer_auth("sk_debug")
            .json(request_body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let response_body: Value = response.json().await.unwrap();
        assert_eq!(status, StatusCode::OK, "{provider}: {response_body}");
        assert_eq!(
            response_body["choices"][0]["message"]["content"],
            "cloud contract answer"
        );
    }
    let requests = upstream.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        2,
        "only one real upstream POST per ingress request"
    );
    let first_body: Value = requests[0].body_json().unwrap();
    let second_body: Value = requests[1].body_json().unwrap();
    assert_eq!(
        first_body, second_body,
        "default and explicit adapter payloads match"
    );
    for request in &requests {
        assert_eq!(request.url.path(), upstream_path);
        let body: Value = request.body_json().unwrap();
        match provider {
            "google" => {
                assert!(
                    body.get("model").is_none(),
                    "Google selects the stripped model in its URL"
                );
                assert_eq!(
                    body["contents"],
                    json!([{"role":"user","parts":[{"text":"cloud contract prompt"}]}])
                );
                assert_eq!(body["generationConfig"]["maxOutputTokens"], 8);
                assert_eq!(
                    request
                        .url
                        .query_pairs()
                        .find(|(key, _)| key == "key")
                        .map(|(_, value)| value.into_owned()),
                    Some(fake_key.clone())
                );
            }
            "anthropic" => {
                assert_eq!(body["model"], upstream_model);
                assert_eq!(
                    body["messages"],
                    json!([{"role":"user","content":[{"type":"text","text":"cloud contract prompt"}]}])
                );
                assert_eq!(body["max_tokens"], 8);
                assert_eq!(
                    request.headers.get("x-api-key").unwrap().to_str().unwrap(),
                    fake_key
                );
                assert_eq!(
                    request
                        .headers
                        .get("anthropic-version")
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    "2023-06-01"
                );
            }
            _ => {
                let mut expected = explicit_payload.clone();
                expected["model"] = json!(upstream_model);
                assert_eq!(body, expected);
                assert_eq!(
                    request
                        .headers
                        .get("authorization")
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    format!("Bearer {fake_key}")
                );
            }
        }
    }
    recorded_once(
        &pool,
        provider,
        &public_model,
        RequestType::Chat,
        &explicit_payload,
    )
    .await;
    lb.stop().await;
}

#[tokio::test]
#[serial_test::serial]
async fn openai_cloud_chat_default_matches_explicit_dispatch_history_and_statistics() {
    chat_adapter_contract("openai").await;
}

#[tokio::test]
#[serial_test::serial]
async fn google_cloud_chat_default_matches_explicit_dispatch_history_and_statistics() {
    chat_adapter_contract("google").await;
}

#[tokio::test]
#[serial_test::serial]
async fn anthropic_cloud_chat_default_matches_explicit_dispatch_history_and_statistics() {
    chat_adapter_contract("anthropic").await;
}

#[tokio::test]
#[serial_test::serial]
async fn anthropic_native_messages_default_matches_explicit_dispatch_history_and_statistics() {
    let upstream = MockServer::start().await;
    let prefix = format!("/{}", Uuid::new_v4());
    let upstream_model = "native-cloud-contract";
    let public_model = format!("anthropic:{upstream_model}");
    let _env = RestoreEnv::set(&[
        ("ANTHROPIC_API_KEY", "test-only-native-cloud-key".into()),
        (
            "ANTHROPIC_API_BASE_URL",
            format!("{}{prefix}", upstream.uri()),
        ),
    ]);
    let response_body = upstream_chat_response("anthropic", upstream_model);
    Mock::given(method("POST"))
        .and(path(format!("{prefix}/v1/messages")))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body.clone()))
        .mount(&upstream)
        .await;
    let pool = create_test_db_pool().await;
    save_default(&pool, &public_model).await;
    let lb = spawn_lb(build_test_router(pool.clone()).await).await;
    let client = client();
    let payload = json!({"messages":[{"role":"user","content":"native cloud prompt"}],"system":"keep this native system","max_tokens":8,"metadata":{"user_id":"cloud-contract"}});
    let mut explicit_payload = payload.clone();
    explicit_payload["model"] = json!(public_model);
    for request_body in [&payload, &explicit_payload] {
        let response = client
            .post(format!("http://{}/v1/messages", lb.addr()))
            .bearer_auth("sk_debug")
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "test-native-beta")
            .json(request_body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            response_body,
            "native response shape is retained"
        );
    }
    let requests = upstream.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    let mut expected = explicit_payload.clone();
    expected["model"] = json!(upstream_model);
    for request in &requests {
        let body: Value = request.body_json().unwrap();
        assert_eq!(body, expected, "only public provider prefix is removed");
        assert_eq!(request.url.path(), format!("{prefix}/v1/messages"));
        for (name, value) in [
            ("x-api-key", "test-only-native-cloud-key"),
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "test-native-beta"),
        ] {
            assert_eq!(request.headers.get(name).unwrap().to_str().unwrap(), value);
        }
    }
    let records = recorded_once(
        &pool,
        "anthropic",
        &public_model,
        RequestType::AnthropicMessages,
        &explicit_payload,
    )
    .await;
    for record in records {
        assert_eq!(record.input_tokens, Some(3));
        assert_eq!(record.output_tokens, Some(2));
        // Existing native extraction preserves the upstream's absent total field.
        assert_eq!(record.total_tokens, None);
    }
    lb.stop().await;
}
