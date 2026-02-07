//! Integration Test: US1 - エンドポイント登録
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! 管理者として、Ollama・vLLM・xLLMなどのエンドポイントを
//! ダッシュボードまたはAPIから登録したい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// US1-シナリオ1: ダッシュボード/APIからエンドポイントを登録し、
/// 一覧に表示されることを確認
#[tokio::test]
async fn test_endpoint_registration_appears_in_list() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録
    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Production Ollama",
            // 外部ネットワーク依存を避け、到達不能でも即時に失敗するローカルURLを使う
            "base_url": "http://127.0.0.1:18134",
            "notes": "Main production server"
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // 一覧で確認
    let list_response = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .expect("list request failed");

    assert_eq!(list_response.status().as_u16(), 200);

    let list: Value = list_response.json().await.unwrap();
    let endpoints = list["endpoints"].as_array().unwrap();
    let found = endpoints
        .iter()
        .any(|e| e["id"].as_str() == Some(endpoint_id));

    assert!(found, "Registered endpoint should appear in list");
}

/// US1-シナリオ2: 登録時に初期状態がpendingになることを確認
#[tokio::test]
async fn test_endpoint_registration_initial_status_pending() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();
    assert_eq!(
        body["status"], "pending",
        "Initial status should be pending"
    );
}

/// US1-シナリオ3: 複数タイプのエンドポイント（Ollama、vLLM、xLLM）を
/// 統一的に登録できることを確認
#[tokio::test]
async fn test_endpoint_registration_multiple_types() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // Ollama
    let ollama_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Ollama Server",
            "base_url": "http://localhost:11434"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(ollama_resp.status().as_u16(), 201);

    // vLLM
    let vllm_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "vLLM Server",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(vllm_resp.status().as_u16(), 201);

    // xLLM
    let xllm_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "xLLM Server",
            "base_url": "http://localhost:9000"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(xllm_resp.status().as_u16(), 201);

    // 全て一覧に表示される
    let list_response = client
        .get(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let list: Value = list_response.json().await.unwrap();
    assert_eq!(list["total"], 3);
}

/// US1-シナリオ4: APIキー付きエンドポイントの登録
/// （APIキーはレスポンスに含まれない）
#[tokio::test]
async fn test_endpoint_registration_with_api_key() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    let response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "OpenAI Compatible",
            // 外部ドメインへの接続待ちでハングし得るため、ローカルURLで代替
            "base_url": "http://127.0.0.1:18080",
            "api_key": "sk-secret-key-12345"
        }))
        .send()
        .await
        .expect("registration request failed");

    assert_eq!(response.status().as_u16(), 201);

    let body: Value = response.json().await.unwrap();

    // api_keyがレスポンスに含まれていないことを確認（セキュリティ）
    assert!(
        body.get("api_key").is_none() || body["api_key"].is_null(),
        "API key should not be included in response"
    );
}
