//! Integration Test: US4 - 接続テスト
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! 管理者として、エンドポイント登録前に接続テストを実行したい。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

/// US4-シナリオ1: 正しいURLで接続テスト成功
#[tokio::test]
async fn test_connection_test_success() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "test-model", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 接続テスト
    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(test_resp.status().as_u16(), 200);

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], true);
    assert!(test_body["latency_ms"].is_number());
    assert!(test_body["endpoint_info"]["model_count"].is_number());
}

/// US4-シナリオ2: 不正なURLで接続テスト失敗
#[tokio::test]
async fn test_connection_test_failure_invalid_url() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Invalid Endpoint",
            "base_url": "http://127.0.0.1:59999"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(test_resp.status().as_u16(), 200);

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], false);
    assert!(test_body["error"].is_string());
    assert!(test_body["latency_ms"].is_null());
}

/// US4-シナリオ3: 認証エラーの検知
#[tokio::test]
async fn test_connection_test_auth_error() {
    let mock = MockServer::start().await;

    // 認証エラーを返す
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {
                "message": "Invalid API key",
                "type": "invalid_request_error"
            }
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Auth Error Endpoint",
            "base_url": mock.uri(),
            "api_key": "invalid-key"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(test_resp.status().as_u16(), 200);

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], false);
    // エラーメッセージに認証関連の情報が含まれる
    let error_msg = test_body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("401")
            || error_msg.to_lowercase().contains("auth")
            || error_msg.to_lowercase().contains("unauthorized"),
        "Error message should indicate authentication failure"
    );
}

/// US4-シナリオ4: 接続テストでモデル数が取得される
#[tokio::test]
async fn test_connection_test_returns_model_count() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "model-1", "object": "model"},
                {"id": "model-2", "object": "model"},
                {"id": "model-3", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Model Count Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["endpoint_info"]["model_count"], 3);
}
