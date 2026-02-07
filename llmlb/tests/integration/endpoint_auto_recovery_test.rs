//! Integration Test: T016c - 自動復旧
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! エンドポイントの自動復旧機能を検証する。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

/// オフライン状態からオンラインへの復旧を検証
#[tokio::test]
async fn test_endpoint_recovery_offline_to_online() {
    let mock = MockServer::start().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（まだモックは応答しない）
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Recovery Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 接続テスト（モックがまだ設定されていないので失敗）
    let test_fail_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_fail_body: Value = test_fail_resp.json().await.unwrap();
    assert_eq!(test_fail_body["success"], false);

    // モックを設定（エンドポイントが復旧）
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "model-1", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    // 再度接続テスト（今度は成功）
    let test_success_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_success_body: Value = test_success_resp.json().await.unwrap();
    assert_eq!(test_success_body["success"], true);

    // ステータスがonlineになっている
    let detail_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    assert_eq!(detail["status"], "online");
}

/// error_countがリセットされることを確認
#[tokio::test]
async fn test_error_count_reset_on_recovery() {
    let mock = MockServer::start().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Error Count Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 複数回の失敗でerror_countが増加
    for _ in 0..3 {
        let _ = client
            .post(format!(
                "http://{}/api/endpoints/{}/test",
                server.addr(),
                endpoint_id
            ))
            .header("authorization", "Bearer sk_debug")
            .send()
            .await
            .unwrap();
    }

    // error_countが増加しているはず
    let detail_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    let error_count_before = detail["error_count"].as_u64().unwrap();
    assert!(error_count_before > 0, "Error count should have increased");

    // エンドポイントを復旧
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    // 成功
    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], true);

    // error_countがリセットされている
    let detail_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    assert_eq!(detail["error_count"], 0, "Error count should be reset");
}

/// last_errorが成功時にクリアされることを確認
#[tokio::test]
async fn test_last_error_cleared_on_recovery() {
    let mock = MockServer::start().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Last Error Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 失敗してlast_errorがセットされる
    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // ダッシュボードAPI（キャッシュ）で last_error がセットされている
    let dash_resp = client
        .get(format!("http://{}/api/dashboard/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let endpoints: Vec<Value> = dash_resp.json().await.unwrap();
    let endpoint = endpoints
        .into_iter()
        .find(|ep| ep["id"].as_str() == Some(endpoint_id))
        .expect("Endpoint should exist in dashboard list");

    assert_eq!(endpoint["status"], "error");
    assert!(endpoint["last_error"].is_string());

    // エンドポイントを復旧
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    // 成功
    let test_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], true);

    // last_errorがクリアされている（ダッシュボードはキャッシュを参照するため）
    let dash_resp = client
        .get(format!("http://{}/api/dashboard/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let endpoints: Vec<Value> = dash_resp.json().await.unwrap();
    let endpoint = endpoints
        .into_iter()
        .find(|ep| ep["id"].as_str() == Some(endpoint_id))
        .expect("Endpoint should exist in dashboard list");

    assert_eq!(endpoint["status"], "online");
    assert!(
        endpoint["last_error"].is_null(),
        "last_error should be cleared"
    );
    assert_eq!(endpoint["error_count"], 0, "error_count should be reset");
}

/// Dashboard APIが接続テスト失敗を即座に反映することを確認
///
/// /api/dashboard/endpoints は EndpointRegistry のキャッシュを返すため、
/// /api/endpoints/:id/test がDB更新だけで終わるとUI上の状態が遅延して見える。
#[tokio::test]
async fn test_dashboard_reflects_test_failure_immediately() {
    let mock = MockServer::start().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Dashboard Failure Reflect Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // wiremockは未マッチのリクエストに404を返すため、接続テストは失敗する
    let test_fail_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_fail_body: Value = test_fail_resp.json().await.unwrap();
    assert_eq!(test_fail_body["success"], false);

    // Dashboard API（キャッシュ）にerrorが反映されている
    let dash_resp = client
        .get(format!("http://{}/api/dashboard/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let endpoints: Vec<Value> = dash_resp.json().await.unwrap();
    let endpoint = endpoints
        .into_iter()
        .find(|ep| ep["id"].as_str() == Some(endpoint_id))
        .expect("Endpoint should exist in dashboard list");

    assert_eq!(endpoint["status"], "error");
    assert!(
        endpoint["last_error"].is_string(),
        "last_error should be set"
    );
    assert!(
        endpoint["error_count"].as_u64().unwrap_or(0) > 0,
        "error_count should be > 0"
    );
}

/// last_seenが更新されることを確認
#[tokio::test]
async fn test_last_seen_updated_on_success() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Last Seen Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 登録直後はlast_seenがnull
    assert!(reg_body["last_seen"].is_null());

    // 接続テスト
    let _ = client
        .post(format!(
            "http://{}/api/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // last_seenが更新されている
    let detail_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    assert!(
        detail["last_seen"].is_string(),
        "last_seen should be updated"
    );
}
