//! Integration Test: US3 - モデル同期
//!
//! SPEC-66555000: llmlb主導エンドポイント登録システム
//!
//! 管理者として、エンドポイントで利用可能なモデルを自動的に取得したい。

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb;

/// US3-シナリオ1: Ollamaエンドポイントからモデル同期
#[tokio::test]
async fn test_sync_models_from_ollama() {
    let mock = MockServer::start().await;

    // Ollamaのモデル一覧レスポンス（/v1/models形式）
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "llama2:7b", "object": "model"},
                {"id": "codellama:13b", "object": "model"},
                {"id": "mistral:7b", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録
    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Ollama Server",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // モデル同期
    let sync_resp = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(sync_resp.status().as_u16(), 200);

    let sync_body: Value = sync_resp.json().await.unwrap();
    let synced_models = sync_body["synced_models"].as_array().unwrap();

    assert_eq!(synced_models.len(), 3);

    // モデルIDの確認
    let model_ids: Vec<&str> = synced_models
        .iter()
        .filter_map(|m| m["model_id"].as_str())
        .collect();
    assert!(model_ids.contains(&"llama2:7b"));
    assert!(model_ids.contains(&"codellama:13b"));
    assert!(model_ids.contains(&"mistral:7b"));
}

/// US3-シナリオ2: vLLMエンドポイントからモデル同期
#[tokio::test]
async fn test_sync_models_from_vllm() {
    let mock = MockServer::start().await;

    // vLLMのOpenAI互換モデル一覧
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "meta-llama/Llama-2-7b-hf", "object": "model", "created": 1234567890, "owned_by": "vllm"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "vLLM Server",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let sync_resp = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(sync_resp.status().as_u16(), 200);

    let sync_body: Value = sync_resp.json().await.unwrap();
    assert_eq!(sync_body["synced_models"].as_array().unwrap().len(), 1);
}

/// US3-シナリオ3: 同期後にエンドポイント詳細でモデルが表示される
#[tokio::test]
async fn test_synced_models_appear_in_endpoint_detail() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "gpt-4", "object": "model"},
                {"id": "gpt-3.5-turbo", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "OpenAI Compatible",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    // モデル同期
    let _ = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // 詳細取得でモデルが含まれる
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
    let models = detail["models"].as_array().unwrap();

    assert_eq!(models.len(), 2);
}

/// US3-シナリオ4: 同期時に追加/削除/更新のカウントが返される
#[tokio::test]
async fn test_sync_returns_change_counts() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"id": "model-1", "object": "model"}]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Change Count Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let sync_resp = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let sync_body: Value = sync_resp.json().await.unwrap();

    // added, removed, updatedフィールドが存在する
    assert!(sync_body["added"].is_number());
    assert!(sync_body["removed"].is_number());
    assert!(sync_body["updated"].is_number());
}
