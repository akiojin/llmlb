//! Integration Test: US2 - 稼働状況監視
//!
//! SPEC-e8e9326e: llmlb主導エンドポイント登録システム
//!
//! 管理者として、登録したエンドポイントの稼働状況をリアルタイムで確認したい。
//!
//! SPEC #585 FR-028 CP-2: 登録 → ヘルスチェック → オフライン検出（TPS 0.0 リセット）
//! → 復帰 → 振り分け対象への再投入 を、アプリと同じ LoadManager に配線した
//! ヘルスチェッカーで検証する。計測と観測はプロキシと管理 API（HTTP）経由で行う。

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use llmlb::balancer::LoadManager;
use llmlb::health::EndpointHealthChecker;
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockGuard, MockServer, ResponseTemplate};

use crate::support::lb::{register_responses_endpoint, spawn_test_lb, spawn_test_lb_with_manager};

const CP2_MODEL: &str = "cp2-lifecycle-model";
/// 上流スタブが返す応答本文。プロキシ経由の応答が当該エンドポイントから届いたことの目印
const CP2_UPSTREAM_REPLY: &str = "reply-from-cp2-upstream";
/// 上流スタブの推論応答の遅延。TPS の計測には 0 より大きい処理時間が必要
const CP2_CHAT_DELAY: Duration = Duration::from_millis(30);

/// US2-シナリオ1: エンドポイント一覧で稼働状況が表示される
#[tokio::test]
async fn test_endpoint_status_displayed_in_list() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(reg_resp.status().as_u16(), 201);

    // 一覧取得
    let response = client
        .get(format!("http://{}/api/endpoints", server.addr()))
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

    let server = spawn_test_lb().await;
    let client = Client::new();

    // モックエンドポイントを登録
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
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

    // 詳細を確認（ステータスがonlineに更新されていることを期待）
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
    // 接続テスト成功後はonlineになる
    assert_eq!(detail["status"], "online");
}

/// US2-シナリオ3: オフラインエンドポイントの検知
#[tokio::test]
async fn test_endpoint_offline_status_detection() {
    // 登録時の自動検出用にモックを起動
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（自動検出で成功）
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Unreachable Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(reg_resp.status().as_u16(), 201);
    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // モックをリセット（接続テストが失敗するようにする）
    mock.reset().await;

    // 接続テストでオフラインを検知
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
    assert_eq!(test_body["success"], false);

    // 詳細を確認（ステータスがoffline/errorになる）
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

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
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
    assert!(
        test_body["latency_ms"].is_number(),
        "Latency should be recorded"
    );
}

/// 上流スタブの `/v1/models` を応答可能にする。返したガードを破棄すると応答しなくなる
async fn mount_models(upstream: &MockServer) -> MockGuard {
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": CP2_MODEL, "object": "model"}]
        })))
        .mount_as_scoped(upstream)
        .await
}

/// 上流スタブの推論 API を応答可能にする。ヘルスチェックの成否とは独立に応答し続ける
async fn mount_chat(upstream: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(CP2_CHAT_DELAY)
                .set_body_json(json!({
                    "id": "chatcmpl-cp2",
                    "object": "chat.completion",
                    "model": CP2_MODEL,
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": CP2_UPSTREAM_REPLY},
                        "finish_reason": "stop"
                    }],
                    "usage": {"prompt_tokens": 5, "completion_tokens": 40, "total_tokens": 45}
                })),
        )
        .mount(upstream)
        .await;
}

/// アプリと同じレジストリ・LoadManager に配線したヘルスチェッカー（起動時の配線と同じ構成）
fn health_checker(load_manager: &LoadManager) -> EndpointHealthChecker {
    EndpointHealthChecker::new(load_manager.endpoint_registry().as_ref().clone())
        .with_load_manager(load_manager.clone())
}

async fn chat_through_proxy(client: &Client, lb_addr: SocketAddr) -> reqwest::Response {
    client
        .post(format!("http://{}/v1/chat/completions", lb_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": CP2_MODEL,
            "messages": [{"role": "user", "content": "ping"}]
        }))
        .send()
        .await
        .expect("chat completion request")
}

async fn fetch_endpoint_status(client: &Client, lb_addr: SocketAddr, endpoint_id: &str) -> String {
    let detail: Value = client
        .get(format!("http://{}/api/endpoints/{}", lb_addr, endpoint_id))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("endpoint detail request")
        .json()
        .await
        .expect("endpoint detail json");
    detail["status"]
        .as_str()
        .expect("endpoint status")
        .to_string()
}

async fn fetch_model_tps(client: &Client, lb_addr: SocketAddr, endpoint_id: &str) -> Vec<Value> {
    let resp = client
        .get(format!(
            "http://{}/api/endpoints/{}/model-tps",
            lb_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("model-tps request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    resp.json().await.expect("model-tps json")
}

/// 振り分けが優先度に用いる TPS。計測値を持たないエンドポイントは 0.0（最低優先）として扱われる
fn routing_tps(entries: &[Value]) -> f64 {
    entries
        .iter()
        .filter_map(|entry| entry["tps"].as_f64())
        .fold(0.0, f64::max)
}

/// 統計記録は応答後に非同期で行われるため、TPS が計測されるまで待つ
async fn wait_for_measured_tps(client: &Client, lb_addr: SocketAddr, endpoint_id: &str) -> f64 {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let tps = routing_tps(&fetch_model_tps(client, lb_addr, endpoint_id).await);
        if tps > 0.0 {
            return tps;
        }
        if Instant::now() > deadline {
            panic!("Timed out waiting for a TPS measurement on endpoint {endpoint_id}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// ヘルスチェックの失敗を重ね、エンドポイントがオフラインと判定されるまで進める
async fn fail_health_checks_until_offline(
    checker: &EndpointHealthChecker,
    client: &Client,
    lb_addr: SocketAddr,
    endpoint_id: &str,
) {
    const MAX_CHECKS: usize = 5;
    let endpoint_uuid = Uuid::parse_str(endpoint_id).expect("endpoint uuid");

    for _ in 0..MAX_CHECKS {
        assert!(
            checker.check_endpoint_by_id(endpoint_uuid).await.is_err(),
            "health check must fail while /v1/models is down"
        );
        if fetch_endpoint_status(client, lb_addr, endpoint_id).await == "offline" {
            return;
        }
    }

    panic!("endpoint {endpoint_id} did not go offline after {MAX_CHECKS} failed health checks");
}

/// FR-028 CP-2: オフライン検出時に、計測済みの TPS が 0.0（未計測）へリセットされる
#[tokio::test]
async fn test_offline_detection_resets_endpoint_tps() {
    let (server, load_manager) = spawn_test_lb_with_manager().await;
    let client = Client::new();
    let upstream = MockServer::start().await;
    let models = mount_models(&upstream).await;
    mount_chat(&upstream).await;
    let endpoint_id = register_responses_endpoint(server.addr(), *upstream.address(), CP2_MODEL)
        .await
        .expect("register upstream endpoint");

    // 計測値を持つ状態にする
    let response = chat_through_proxy(&client, server.addr()).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let measured = wait_for_measured_tps(&client, server.addr(), &endpoint_id).await;
    assert!(measured > 0.0, "TPS must be measured before going offline");

    // ヘルスチェックの応答だけを止めてオフラインにする
    drop(models);
    let checker = health_checker(&load_manager);
    fail_health_checks_until_offline(&checker, &client, server.addr(), &endpoint_id).await;

    let entries = fetch_model_tps(&client, server.addr(), &endpoint_id).await;
    assert!(
        entries.is_empty(),
        "offline detection must discard the measured TPS, got {entries:?}"
    );
    assert_eq!(routing_tps(&entries), 0.0);
}

/// FR-028 CP-2: オフラインのエンドポイントは振り分け対象から外れ、復帰すると再投入される
#[tokio::test]
async fn test_recovered_endpoint_is_routable_again() {
    let (server, load_manager) = spawn_test_lb_with_manager().await;
    let client = Client::new();
    let upstream = MockServer::start().await;
    let models = mount_models(&upstream).await;
    mount_chat(&upstream).await;
    let endpoint_id = register_responses_endpoint(server.addr(), *upstream.address(), CP2_MODEL)
        .await
        .expect("register upstream endpoint");
    let endpoint_uuid = Uuid::parse_str(&endpoint_id).expect("endpoint uuid");

    let response = chat_through_proxy(&client, server.addr()).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // 推論 API は応答できるまま、ヘルスチェックの応答だけを止めてオフラインにする
    drop(models);
    let checker = health_checker(&load_manager);
    fail_health_checks_until_offline(&checker, &client, server.addr(), &endpoint_id).await;

    // 上流の推論 API は生きているので、この拒否は上流の障害ではなく llmlb の振り分けによる。
    // オンラインの提供元がないモデルは「存在しないモデル」として拒否される
    let response = chat_through_proxy(&client, server.addr()).await;
    assert_eq!(
        response.status(),
        reqwest::StatusCode::NOT_FOUND,
        "an offline endpoint must not receive requests"
    );

    // 復帰
    let _models = mount_models(&upstream).await;
    checker
        .check_endpoint_by_id(endpoint_uuid)
        .await
        .expect("health check must succeed after recovery");
    assert_eq!(
        fetch_endpoint_status(&client, server.addr(), &endpoint_id).await,
        "online"
    );

    let response = chat_through_proxy(&client, server.addr()).await;
    assert_eq!(
        response.status(),
        reqwest::StatusCode::OK,
        "a recovered endpoint must receive requests again"
    );
    let body: Value = response.json().await.expect("chat completion json");
    assert_eq!(
        body["choices"][0]["message"]["content"], CP2_UPSTREAM_REPLY,
        "the response must come from the recovered endpoint"
    );
}
