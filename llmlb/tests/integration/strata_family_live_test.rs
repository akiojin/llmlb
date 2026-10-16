//! Issues #908/#924: opt-in registration and inference against real upstream servers.
//!
//! These tests never download models or start a mock engine. Start the upstream
//! server, set its URL, then explicitly select the ignored test (see README).

use crate::support::http::TestServer;
use crate::support::lb::spawn_test_lb_with_db;
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

async fn check_live_endpoint(
    prefix: &str,
    expected: &str,
) -> (TestServer, Client, String, uuid::Uuid) {
    let base_url = std::env::var(format!("LLMLB_{prefix}_URL"))
        .expect("set the upstream URL before explicitly running this ignored test");
    assert!(
        !base_url.trim().is_empty(),
        "upstream URL must not be empty"
    );
    let api_key = std::env::var(format!("LLMLB_{prefix}_API_KEY")).ok();
    let client = Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .unwrap();
    let detected =
        llmlb::detection::detect_endpoint_type_with_client(&client, &base_url, api_key.as_deref())
            .await
            .expect("detect real upstream endpoint");
    assert_eq!(detected.endpoint_type.as_str(), expected);
    println!(
        "live detection: endpoint_type={expected}, reason={}",
        detected.reason
    );

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let password = llmlb::auth::password::hash_password("live-smoke-only").unwrap();
    let admin = llmlb::db::users::create(&db_pool, "live-admin", &password, UserRole::Admin, false)
        .await
        .unwrap();
    let admin_key = llmlb::db::api_keys::create(
        &db_pool,
        "live-smoke-admin",
        admin.id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .unwrap();
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .bearer_auth(&admin_key.key)
        .json(&json!({
            "name": expected,
            "base_url": base_url,
            "api_key": api_key,
            "inference_timeout_secs": 600
        }))
        .send()
        .await
        .expect("register real upstream endpoint in isolated llmlb");
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status.as_u16(), 201, "registration: {body}");
    assert_eq!(body["endpoint_type"], expected);
    let id = uuid::Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let stored = llmlb::db::endpoints::get_endpoint(&db_pool, id)
        .await
        .unwrap()
        .expect("registered endpoint is persisted");
    assert_eq!(stored.endpoint_type.as_str(), expected);
    println!(
        "live registration: HTTP {status}, endpoint_type={expected}, SQLite={}",
        stored.endpoint_type
    );
    println!(
        "registration evidence: {}",
        json!({"id": id, "base_url": base_url, "endpoint_type": body["endpoint_type"],
            "api_key_configured": stored.api_key.is_some()})
    );
    (server, client, admin_key.key, id)
}

async fn check_live_inference(prefix: &str, expected: &str, require_key: bool) {
    if require_key {
        assert!(
            std::env::var(format!("LLMLB_{prefix}_API_KEY"))
                .is_ok_and(|key| !key.trim().is_empty()),
            "authenticated smoke requires a nonempty upstream API key"
        );
    }
    let (server, client, key, id) = check_live_endpoint(prefix, expected).await;
    if require_key {
        let upstream = std::env::var(format!("LLMLB_{prefix}_URL")).unwrap();
        let response = client
            .get(format!("{}/v1/models", upstream.trim_end_matches('/')))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            401,
            "upstream must enforce its API key"
        );
        println!("upstream GET /v1/models without key: HTTP 401");
    }
    let lb_url = format!("http://{}", server.addr());

    // Registration schedules health/model synchronization in the background.
    // Explicit API calls make this smoke independent of its scheduling.
    for action in ["test", "sync"] {
        let response = client
            .post(format!("{lb_url}/api/endpoints/{id}/{action}"))
            .bearer_auth(&key)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body: Value = response.json().await.unwrap();
        assert_eq!(status.as_u16(), 200, "{action}: {body}");
        if action == "test" {
            assert_eq!(body["success"], true, "health check: {body}");
        }
    }

    let response = client
        .get(format!("{lb_url}/v1/models"))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let models: Value = response.json().await.unwrap();
    println!("GET /v1/models: HTTP {status}\n{models}");
    assert_eq!(status.as_u16(), 200);
    let available = models["data"].as_array().expect("OpenAI model list");
    let model = std::env::var(format!("LLMLB_{prefix}_MODEL")).unwrap_or_else(|_| {
        assert_eq!(
            available.len(),
            1,
            "set LLMLB_{prefix}_MODEL for multiple models"
        );
        available[0]["id"].as_str().unwrap().to_owned()
    });
    assert!(available.iter().any(|entry| entry["id"] == model));

    for stream in [false, true] {
        let mut request = json!({
            "model": model,
            "messages": [{"role": "user", "content": "Reply with exactly: hello"}],
            "max_tokens": 64,
            "reasoning_effort": "none",
            "stream": stream
        });
        if stream {
            request["stream_options"] = json!({"include_usage": true});
        }
        println!("POST /v1/chat/completions request: {request}");
        let response = client
            .post(format!("{lb_url}/v1/chat/completions"))
            .bearer_auth(&key)
            .json(&request)
            .send()
            .await
            .expect("inference through llmlb");
        let status = response.status();
        let content_type = response.headers()["content-type"]
            .to_str()
            .unwrap()
            .to_owned();
        let body = response.text().await.expect("complete inference response");
        println!("response: HTTP {status}, Content-Type: {content_type}\n{body}");
        assert_eq!(status.as_u16(), 200, "inference: {body}");
        if stream {
            assert!(content_type.starts_with("text/event-stream"));
            let data: Vec<_> = body
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect();
            assert_eq!(data.last().copied(), Some("[DONE]"), "SSE terminal event");
            let mut content = String::new();
            let mut finished = false;
            let mut usage = None;
            for event in &data[..data.len() - 1] {
                let chunk: Value = serde_json::from_str(event).expect("SSE JSON event");
                assert_eq!(chunk["model"], model);
                assert_eq!(chunk["object"], "chat.completion.chunk");
                if let Some(text) = chunk["choices"][0]["delta"]["content"].as_str() {
                    content.push_str(text);
                }
                finished |= chunk["choices"][0]["finish_reason"].as_str().is_some();
                if chunk["usage"].is_object() {
                    usage = Some(chunk["usage"].clone());
                }
            }
            assert!(
                !content.trim().is_empty(),
                "stream must contain generated text"
            );
            assert!(finished, "stream must contain a finish reason");
            check_usage(&usage.expect("include_usage must produce usage"));
        } else {
            let completion: Value = serde_json::from_str(&body).unwrap();
            assert_eq!(completion["model"], model);
            assert_eq!(completion["object"], "chat.completion");
            assert!(completion["choices"][0]["message"]["content"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()));
            assert!(completion["choices"][0]["finish_reason"].as_str().is_some());
            check_usage(&completion["usage"]);
        }
    }
}

fn check_usage(usage: &Value) {
    let prompt = usage["prompt_tokens"].as_u64().expect("prompt tokens");
    let completion = usage["completion_tokens"]
        .as_u64()
        .expect("completion tokens");
    assert!(prompt > 0);
    assert!(completion > 0);
    assert_eq!(usage["total_tokens"].as_u64(), Some(prompt + completion));
}

#[tokio::test]
#[ignore = "requires a real Strata server and LLMLB_STRATA_URL"]
async fn test_live_strata_identification_and_registration() {
    check_live_endpoint("STRATA", "strata").await;
}

#[tokio::test]
#[ignore = "requires a real Project Maya server and LLMLB_MAYA_URL"]
async fn test_live_project_maya_identification_and_registration() {
    check_live_endpoint("MAYA", "project_maya").await;
}

#[tokio::test]
#[ignore = "requires a loaded Strata server and LLMLB_STRATA_URL"]
async fn test_live_strata_inference_through_llmlb() {
    check_live_inference("STRATA", "strata", false).await;
}

#[tokio::test]
#[ignore = "requires a loaded Project Maya server and LLMLB_MAYA_URL"]
async fn test_live_project_maya_inference_through_llmlb() {
    check_live_inference("MAYA", "project_maya", false).await;
}

#[tokio::test]
#[ignore = "requires a key-protected Project Maya, LLMLB_MAYA_URL and LLMLB_MAYA_API_KEY"]
async fn test_live_project_maya_authenticated_inference_through_llmlb() {
    check_live_inference("MAYA", "project_maya", true).await;
}
