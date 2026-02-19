//! Integration Test: US5 - 管理操作
//!
//! SPEC-e8e9326e: llmlb主導エンドポイント登録システム
//!
//! 管理者として、登録済みエンドポイントの編集・削除を行いたい。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

async fn start_detectable_endpoint_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model"}]
        })))
        .mount(&server)
        .await;
    server
}

/// US5-シナリオ1: エンドポイント名の更新
#[tokio::test]
async fn test_update_endpoint_name() {
    let server = spawn_test_lb().await;
    let client = Client::new();
    let mock = start_detectable_endpoint_server().await;

    // エンドポイント登録
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Original Name",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 名前を更新
    let update_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Updated Name"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(update_resp.status().as_u16(), 200);

    let update_body: Value = update_resp.json().await.unwrap();
    assert_eq!(update_body["name"], "Updated Name");

    // 一覧でも更新されている
    let list_resp = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = list_resp.json().await.unwrap();
    let endpoint = list["endpoints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"].as_str() == Some(endpoint_id))
        .unwrap();

    assert_eq!(endpoint["name"], "Updated Name");
}

/// US5-シナリオ2: ヘルスチェック間隔の更新
#[tokio::test]
async fn test_update_health_check_interval() {
    let server = spawn_test_lb().await;
    let client = Client::new();
    let mock = start_detectable_endpoint_server().await;

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Interval Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();
    assert_eq!(reg_body["health_check_interval_secs"], 30); // デフォルト

    // 間隔を更新
    let update_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "health_check_interval_secs": 120
        }))
        .send()
        .await
        .unwrap();

    let update_body: Value = update_resp.json().await.unwrap();
    assert_eq!(update_body["health_check_interval_secs"], 120);
}

/// US5-シナリオ3: エンドポイントの削除
#[tokio::test]
async fn test_delete_endpoint() {
    let server = spawn_test_lb().await;
    let client = Client::new();
    let mock = start_detectable_endpoint_server().await;

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "To Be Deleted",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 削除
    let delete_resp = client
        .delete(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(delete_resp.status().as_u16(), 204);

    // 一覧から消えている
    let list_resp = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = list_resp.json().await.unwrap();
    assert_eq!(list["total"], 0);
}

/// US5-シナリオ4: notesの追加・更新・削除
#[tokio::test]
async fn test_manage_endpoint_notes() {
    let server = spawn_test_lb().await;
    let client = Client::new();
    let mock = start_detectable_endpoint_server().await;

    // notesなしで登録
    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Notes Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();
    assert!(reg_body["notes"].is_null());

    // notesを追加
    let add_notes_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "notes": "Production server - do not disable"
        }))
        .send()
        .await
        .unwrap();

    let add_notes_body: Value = add_notes_resp.json().await.unwrap();
    assert_eq!(
        add_notes_body["notes"],
        "Production server - do not disable"
    );

    // notesを削除（nullで更新）
    let remove_notes_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "notes": null
        }))
        .send()
        .await
        .unwrap();

    let remove_notes_body: Value = remove_notes_resp.json().await.unwrap();
    assert!(remove_notes_body["notes"].is_null());
}

/// US5-シナリオ5: 複数フィールドの同時更新
#[tokio::test]
async fn test_update_multiple_fields() {
    let server = spawn_test_lb().await;
    let client = Client::new();
    let mock = start_detectable_endpoint_server().await;

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Multi Update Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // 複数フィールドを同時更新
    let update_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "New Name",
            "health_check_interval_secs": 60,
            "notes": "Updated notes"
        }))
        .send()
        .await
        .unwrap();

    let update_body: Value = update_resp.json().await.unwrap();
    assert_eq!(update_body["name"], "New Name");
    assert_eq!(update_body["health_check_interval_secs"], 60);
    assert_eq!(update_body["notes"], "Updated notes");
}
