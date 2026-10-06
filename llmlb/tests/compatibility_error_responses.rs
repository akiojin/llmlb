//! Issue #830: compatibility handler errors must reach structured HTTP exits.

use llmlb::common::auth::UserRole;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use serial_test::serial;
use uuid::Uuid;

#[path = "support/mod.rs"]
mod support;

use support::lb::{spawn_test_lb_with_db, test_jwt_secret};

async fn assert_compatibility_error(path: &str, payload: Value, close_database: bool) {
    let (server, pool) = spawn_test_lb_with_db().await;
    if close_database {
        pool.close().await;
    }
    let response = Client::new()
        .post(format!("http://{}{path}", server.addr()))
        .bearer_auth("sk_debug")
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await
        .expect("HTTP response");
    let status = response.status();
    let body: Value = response.json().await.expect("JSON error body");
    server.stop().await;
    println!("{path} status={status} body={body}");
    assert_eq!(
        status,
        if close_database {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::BAD_REQUEST
        },
        "{path}: {body}"
    );
    assert!(body["error"].is_object(), "{path}: {body}");
    for field in ["message", "type", "code"] {
        assert!(
            body["error"][field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{path}: error.{field} missing in {body}"
        );
    }
    assert_eq!(body["error"]["code"], status.as_u16().to_string());
    if close_database {
        assert_eq!(body["error"]["message"], "Database error");
    }
    if path == "/v1/messages" {
        assert_eq!(body["type"], "error", "preserve Anthropic envelope");
    }
}

#[tokio::test]
#[serial]
async fn chat_missing_model_returns_structured_error() {
    assert_compatibility_error(
        "/v1/chat/completions",
        json!({"messages": [{"role": "user", "content": "hello"}]}),
        false,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn completions_missing_model_returns_structured_error() {
    assert_compatibility_error("/v1/completions", json!({"prompt": "hello"}), false).await;
}

#[tokio::test]
#[serial]
async fn completions_invalid_model_returns_structured_error() {
    assert_compatibility_error(
        "/v1/completions",
        json!({"model": "bad:", "prompt": "test"}),
        false,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn speech_invalid_model_returns_structured_error() {
    assert_compatibility_error(
        "/v1/audio/speech",
        json!({"model": "bad:", "input": "hello", "voice": "alloy"}),
        false,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn images_invalid_model_returns_structured_error() {
    assert_compatibility_error(
        "/v1/images/generations",
        json!({"model": "bad:", "prompt": "a cat"}),
        false,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn messages_database_failure_returns_structured_error() {
    assert_compatibility_error(
        "/v1/messages",
        json!({"model": "test-model", "max_tokens": 16,
            "messages": [{"role": "user", "content": "hello"}]}),
        true,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn embeddings_invalid_model_returns_structured_error() {
    assert_compatibility_error(
        "/v1/embeddings",
        json!({"model": "bad:", "input": "hello"}),
        false,
    )
    .await;
}

#[tokio::test]
#[serial]
async fn model_catalog_database_failure_keeps_management_exit() {
    let (server, pool) = spawn_test_lb_with_db().await;
    pool.close().await;
    let jwt = llmlb::auth::jwt::create_jwt(
        &Uuid::nil().to_string(),
        UserRole::Admin,
        &test_jwt_secret(),
        false,
        0,
    )
    .expect("test JWT");
    let client = Client::new();
    for (path, token, structured) in [
        ("/v1/models", "sk_debug", true),
        ("/v1/models/test-model", "sk_debug", true),
        ("/api/dashboard/playground/models", jwt.as_str(), false),
    ] {
        let response = client
            .get(format!("http://{}{path}", server.addr()))
            .bearer_auth(token)
            .send()
            .await
            .expect("HTTP response");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: Value = response.json().await.expect("JSON body");
        println!("{path} body={body}");
        if structured {
            assert_eq!(body["error"]["message"], "Database error");
            assert_eq!(body["error"]["type"], "server_error");
            assert_eq!(body["error"]["code"], "500");
        } else {
            assert_eq!(body, json!({"error": "Database error"}));
        }
    }
    server.stop().await;
}

#[tokio::test]
#[serial]
async fn dashboard_chat_validation_keeps_flat_error() {
    let (server, _) = spawn_test_lb_with_db().await;
    let jwt = llmlb::auth::jwt::create_jwt(
        &Uuid::nil().to_string(),
        UserRole::Admin,
        &test_jwt_secret(),
        false,
        0,
    )
    .expect("test JWT");
    for path in [
        "/api/dashboard/playground/chat/completions",
        "/api/dashboard/playground/load-test/chat/completions",
    ] {
        let response = Client::new()
            .post(format!("http://{}{path}", server.addr()))
            .bearer_auth(&jwt)
            .json(&json!({"messages": [{"role": "user", "content": "hello"}]}))
            .send()
            .await
            .expect("HTTP response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value = response.json().await.expect("JSON body");
        assert_eq!(
            body,
            json!({"error": "`model` field is required for OpenAI-compatible requests"})
        );
    }
    server.stop().await;
}

#[tokio::test]
#[serial]
async fn cloud_connection_failure_returns_safe_structured_error() {
    struct RestoreEnvironment(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for RestoreEnvironment {
        fn drop(&mut self) {
            for (key, previous) in &self.0 {
                match previous {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    let _restore = RestoreEnvironment(
        ["OPENAI_API_KEY", "OPENAI_BASE_URL"]
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect(),
    );
    std::env::set_var("OPENAI_API_KEY", "test-cloud-key");
    std::env::set_var("OPENAI_BASE_URL", "http://127.0.0.1:0/private/endpoint");
    let (server, _) = spawn_test_lb_with_db().await;
    let response = Client::new()
        .post(format!("http://{}/v1/chat/completions", server.addr()))
        .bearer_auth("sk_debug")
        .json(&json!({"model": "openai:test-model",
            "messages": [{"role": "user", "content": "hello"}]}))
        .send()
        .await
        .expect("HTTP response");
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body: Value = response.json().await.expect("JSON error body");
    server.stop().await;
    assert_eq!(
        body,
        json!({"error": {
            "message": "Backend service unavailable", "type": "service_unavailable", "code": "502"
        }})
    );
    let serialized = body.to_string();
    for private in ["127.0.0.1", "/private/endpoint", "test-cloud-key"] {
        assert!(!serialized.contains(private), "must not expose {private}");
    }
}
