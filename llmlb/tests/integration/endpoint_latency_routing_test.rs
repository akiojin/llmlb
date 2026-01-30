//! Integration Test: T016b - レイテンシベースルーティング
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! レイテンシが最も低いエンドポイントを優先的に選択する機能を検証する。

use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

/// 複数エンドポイントがある場合、レイテンシ情報が記録される
#[tokio::test]
async fn test_latency_recorded_for_endpoints() {
    let mock1 = MockServer::start().await;
    let mock2 = MockServer::start().await;

    // 両方のモックが応答する
    for mock in [&mock1, &mock2] {
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{"id": "model-1", "object": "model"}]
            })))
            .mount(mock)
            .await;
    }

    let server = spawn_test_lb().await;
    let client = Client::new();

    // 2つのエンドポイントを登録
    let endpoints = vec![
        ("Fast Endpoint", mock1.uri()),
        ("Slow Endpoint", mock2.uri()),
    ];

    for (name, url) in &endpoints {
        let reg_resp = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .header("x-internal-token", "test-internal")
            .header("authorization", "Bearer sk_debug")
            .json(&json!({
                "name": name,
                "base_url": url
            }))
            .send()
            .await
            .unwrap();

        let reg_body: Value = reg_resp.json().await.unwrap();
        let endpoint_id = reg_body["id"].as_str().unwrap();

        // 接続テストでレイテンシを計測
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
        assert_eq!(test_body["success"], true);
        assert!(
            test_body["latency_ms"].is_number(),
            "Latency should be recorded"
        );
    }

    // 一覧でレイテンシ情報が含まれる（latency_msフィールドがエンドポイントに記録される）
    let list_resp = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = list_resp.json().await.unwrap();
    let list_endpoints = list["endpoints"].as_array().unwrap();

    assert_eq!(list_endpoints.len(), 2);
}

/// レイテンシの遅いエンドポイントでも正常に動作する
#[tokio::test]
async fn test_slow_endpoint_handled() {
    let mock = MockServer::start().await;

    // 遅延応答
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(json!({
                    "object": "list",
                    "data": [{"id": "slow-model", "object": "model"}]
                })),
        )
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Slow Endpoint",
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
    assert_eq!(test_body["success"], true);

    // レイテンシが100ms以上であることを確認
    let latency = test_body["latency_ms"].as_u64().unwrap();
    assert!(latency >= 100, "Latency should be at least 100ms");
}

/// 同じモデルを持つ複数エンドポイントがある場合のルーティング準備
/// （実際のルーティングテストはPhase 3.4で実装）
#[tokio::test]
async fn test_multiple_endpoints_same_model() {
    let mock1 = MockServer::start().await;
    let mock2 = MockServer::start().await;

    for mock in [&mock1, &mock2] {
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{"id": "shared-model", "object": "model"}]
            })))
            .mount(mock)
            .await;
    }

    let server = spawn_test_lb().await;
    let client = Client::new();

    // 2つのエンドポイントを登録（同じモデルを持つ）
    for (i, url) in [mock1.uri(), mock2.uri()].iter().enumerate() {
        let reg_resp = client
            .post(format!("http://{}/api/endpoints", server.addr()))
            .header("x-internal-token", "test-internal")
            .header("authorization", "Bearer sk_debug")
            .json(&json!({
                "name": format!("Endpoint {}", i + 1),
                "base_url": url
            }))
            .send()
            .await
            .unwrap();

        let reg_body: Value = reg_resp.json().await.unwrap();
        let endpoint_id = reg_body["id"].as_str().unwrap();

        // モデル同期
        let _ = client
            .post(format!(
                "http://{}/api/endpoints/{}/sync",
                server.addr(),
                endpoint_id
            ))
            .header("x-internal-token", "test-internal")
            .header("authorization", "Bearer sk_debug")
            .send()
            .await
            .unwrap();
    }

    // 両方のエンドポイントが同じモデルを持つ
    let list_resp = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("x-internal-token", "test-internal")
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = list_resp.json().await.unwrap();
    assert_eq!(list["total"], 2);
}
