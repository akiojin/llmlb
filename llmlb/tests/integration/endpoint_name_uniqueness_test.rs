//! Integration Test: T016a - 名前重複検証
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! エンドポイント名の一意性を検証する。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

/// OpenAI互換として検出されるモックサーバーを作成するヘルパー
async fn create_openai_compatible_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "test-model", "object": "model"}]
        })))
        .mount(&mock)
        .await;
    mock
}

/// 同じ名前のエンドポイントを登録しようとすると400エラー
#[tokio::test]
async fn test_duplicate_name_rejected() {
    let mock1 = create_openai_compatible_mock().await;
    let mock2 = create_openai_compatible_mock().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // 最初のエンドポイント登録
    let first_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production Ollama",
            "base_url": mock1.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(first_resp.status().as_u16(), 201);

    // 同じ名前で異なるURLのエンドポイントを登録（名前重複）
    let dup_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production Ollama",
            "base_url": mock2.uri()
        }))
        .send()
        .await
        .unwrap();

    // 名前重複は400 Bad Request（URLの重複は409 Conflict）
    assert_eq!(dup_resp.status().as_u16(), 400);
}

/// 大文字小文字を区別して名前を検証
#[tokio::test]
async fn test_name_case_sensitivity() {
    let mock1 = create_openai_compatible_mock().await;
    let mock2 = create_openai_compatible_mock().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // 最初のエンドポイント登録
    let _ = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production",
            "base_url": mock1.uri()
        }))
        .send()
        .await
        .unwrap();

    // 大文字小文字が異なる名前は別物として扱う
    let diff_case_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "PRODUCTION",
            "base_url": mock2.uri()
        }))
        .send()
        .await
        .unwrap();

    // 大文字小文字が異なれば別のエンドポイントとして登録可能
    assert_eq!(diff_case_resp.status().as_u16(), 201);
}

/// 削除後に同じ名前を再利用可能
#[tokio::test]
async fn test_name_reusable_after_deletion() {
    let mock1 = create_openai_compatible_mock().await;
    let mock2 = create_openai_compatible_mock().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録
    let first_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Reusable Name",
            "base_url": mock1.uri()
        }))
        .send()
        .await
        .unwrap();

    let first_body: Value = first_resp.json().await.unwrap();
    let endpoint_id = first_body["id"].as_str().unwrap();

    // 削除
    let _ = client
        .delete(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // 同じ名前で再登録可能
    let reuse_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Reusable Name",
            "base_url": mock2.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(reuse_resp.status().as_u16(), 201);
}

/// 更新時に他のエンドポイントの名前と重複を防止
#[tokio::test]
async fn test_update_name_uniqueness() {
    let mock1 = create_openai_compatible_mock().await;
    let mock2 = create_openai_compatible_mock().await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // 2つのエンドポイントを登録
    let _ = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint A",
            "base_url": mock1.uri()
        }))
        .send()
        .await
        .unwrap();

    let second_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint B",
            "base_url": mock2.uri()
        }))
        .send()
        .await
        .unwrap();

    let second_body: Value = second_resp.json().await.unwrap();
    let endpoint_b_id = second_body["id"].as_str().unwrap();

    // Endpoint Bの名前をEndpoint Aに変更しようとする
    let update_resp = client
        .put(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_b_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Endpoint A"
        }))
        .send()
        .await
        .unwrap();

    // 重複は400
    assert_eq!(update_resp.status().as_u16(), 400);
}
