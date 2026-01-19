//! Integration Test: US2 - 稼働状況監視
//!
//! SPEC-66555000: ルーター主導エンドポイント登録システム
//!
//! 管理者として、登録したエンドポイントの稼働状況をリアルタイムで確認したい。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::router::spawn_test_router;

/// US2-シナリオ1: エンドポイント一覧で稼働状況が表示される
#[tokio::test]
async fn test_endpoint_status_displayed_in_list() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // エンドポイント登録
    let _ = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();

    // 一覧取得
    let response = client
        .get(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = response.json().await.unwrap();
    let endpoints = list["endpoints"].as_array().unwrap();
    let endpoint = &endpoints[0];

    // statusフィールドが存在する
    assert!(endpoint["status"].is_string());
}

/// US2-シナリオ2: オンラインエンドポイントのステータス確認
#[tokio::test]
async fn test_endpoint_online_status_after_health_check() {
    let mock = MockServer::start().await;

    // モックエンドポイントがヘルスチェックに応答
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_router().await;
    let client = Client::new();

    // モックエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Mock Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 接続テストでステータスを更新
    let test_resp = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], true);

    // 詳細を確認（ステータスがonlineに更新されていることを期待）
    let detail_resp = client
        .get(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    // 接続テスト成功後はonlineになる
    assert_eq!(detail["status"], "online");
}

/// US2-シナリオ3: オフラインエンドポイントの検知
#[tokio::test]
async fn test_endpoint_offline_status_detection() {
    let server = spawn_test_router().await;
    let client = Client::new();

    // 到達不能なエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Unreachable Endpoint",
            "base_url": "http://127.0.0.1:59999"
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 接続テストでオフラインを検知
    let test_resp = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let test_body: Value = test_resp.json().await.unwrap();
    assert_eq!(test_body["success"], false);

    // 詳細を確認（ステータスがoffline/errorになる）
    let detail_resp = client
        .get(format!(
            "http://{}/v0/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let detail: Value = detail_resp.json().await.unwrap();
    // 接続失敗後はofflineまたはerrorになる
    assert!(
        detail["status"] == "offline" || detail["status"] == "error",
        "Status should be offline or error after failed connection"
    );
}

/// US2-シナリオ4: レイテンシの記録
#[tokio::test]
async fn test_endpoint_latency_recorded() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_router().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Latency Test",
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
            "http://{}/v0/endpoints/{}/test",
            server.addr(),
            endpoint_id
        ))
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
