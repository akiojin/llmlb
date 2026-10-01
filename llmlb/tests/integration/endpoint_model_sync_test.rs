//! Integration Test: US3 - モデル同期
//!
//! SPEC-e8e9326e: llmlb主導エンドポイント登録システム
//!
//! 管理者として、エンドポイントで利用可能なモデルを自動的に取得したい。

use reqwest::Client;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
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
        .post(format!("http://{}/api/endpoints", server.addr()))
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
            "http://{}/api/endpoints/{}/sync",
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
        .post(format!("http://{}/api/endpoints", server.addr()))
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
            "http://{}/api/endpoints/{}/sync",
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
        .post(format!("http://{}/api/endpoints", server.addr()))
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
            "http://{}/api/endpoints/{}/sync",
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
            "http://{}/api/endpoints/{}",
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
        .post(format!("http://{}/api/endpoints", server.addr()))
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
            "http://{}/api/endpoints/{}/sync",
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

/// US3-シナリオ5: エンドポイント登録直後に接続チェック＆モデル同期が自動実行される
#[tokio::test]
async fn test_auto_test_and_sync_on_create() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": "auto-model-1", "object": "model"},
                {"id": "auto-model-2", "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Auto Sync Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    let reg_body: Value = reg_resp.json().await.unwrap();
    let endpoint_id = reg_body["id"].as_str().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let models_resp = client
            .get(format!(
                "http://{}/api/endpoints/{}/models",
                server.addr(),
                endpoint_id
            ))
            .header("authorization", "Bearer sk_debug")
            .send()
            .await
            .unwrap();

        if models_resp.status().is_success() {
            let body: Value = models_resp.json().await.unwrap();
            if let Some(models) = body["models"].as_array() {
                if models.iter().any(|m| m["model_id"] == "auto-model-1") {
                    break;
                }
            }
        }

        if Instant::now() > deadline {
            panic!("Timed out waiting for auto sync to complete");
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let endpoint_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let endpoint_body: Value = endpoint_resp.json().await.unwrap();
    assert_eq!(endpoint_body["status"], "online");
}

/// FR-028 CP-4: エンドポイントが報告したエンジン固有のモデルIDが、
/// カタログ反映後にcanonical名として解決される
#[tokio::test]
async fn test_synced_engine_model_id_resolves_to_canonical_name() {
    const ENGINE_MODEL_ID: &str = "gpt-oss:20b";
    const CANONICAL_NAME: &str = "openai/gpt-oss-20b";
    const UNMAPPED_MODEL_ID: &str = "cp4-unmapped-model:latest";
    const CHAT_PROMPT: &str = "cp4-canonical-ping";

    let mock = MockServer::start().await;

    // /api/tags に応答するエンドポイントはOllamaとして判別される
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{"name": ENGINE_MODEL_ID}, {"name": UNMAPPED_MODEL_ID}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [
                {"id": ENGINE_MODEL_ID, "object": "model"},
                {"id": UNMAPPED_MODEL_ID, "object": "model"}
            ]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-cp4",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "pong"},
                "finish_reason": "stop"
            }]
        })))
        .mount(&mock)
        .await;

    let server = spawn_test_lb().await;
    let client = Client::new();

    let reg_resp = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Canonical Sync Test",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(reg_resp.status().as_u16(), 201);

    let reg_body: Value = reg_resp.json().await.unwrap();
    assert_eq!(reg_body["endpoint_type"], "ollama");
    let endpoint_id = reg_body["id"].as_str().unwrap();

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
    assert_eq!(test_resp.status().as_u16(), 200);

    // 同期: エンジン固有IDがcanonical名へ正規化される（マッピングに無いIDは正規化されない）
    let sync_resp = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(sync_resp.status().as_u16(), 200);

    let sync_body: Value = sync_resp.json().await.unwrap();
    let synced_canonical_of = |model_id: &str| -> Value {
        sync_body["synced_models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["model_id"] == model_id)
            .unwrap_or_else(|| panic!("{model_id} missing from sync response: {sync_body}"))
            ["canonical_name"]
            .clone()
    };
    assert_eq!(synced_canonical_of(ENGINE_MODEL_ID), CANONICAL_NAME);
    assert!(synced_canonical_of(UNMAPPED_MODEL_ID).is_null());

    // カタログ反映（永続化されたエンドポイントのモデル一覧）
    let models_resp = client
        .get(format!(
            "http://{}/api/endpoints/{}/models",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(models_resp.status().as_u16(), 200);

    let models_body: Value = models_resp.json().await.unwrap();
    let stored = models_body["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["model_id"] == ENGINE_MODEL_ID)
        .unwrap_or_else(|| panic!("{ENGINE_MODEL_ID} missing from endpoint models: {models_body}"));
    assert_eq!(stored["canonical_name"], CANONICAL_NAME);

    // カタログ反映（/v1/models）: canonical名で公開され、エンジン固有IDはエイリアスになる
    let catalog_resp = client
        .get(format!("http://{}/v1/models", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(catalog_resp.status().as_u16(), 200);

    let catalog: Value = catalog_resp.json().await.unwrap();
    let canonical_entry = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == CANONICAL_NAME)
        .unwrap_or_else(|| panic!("{CANONICAL_NAME} missing from /v1/models: {catalog}"));
    assert_eq!(canonical_entry["canonical_name"], CANONICAL_NAME);
    assert_eq!(canonical_entry["is_canonical"], true);
    assert!(canonical_entry["endpoint_ids"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == endpoint_id));
    assert!(
        canonical_entry["aliases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|alias| alias == ENGINE_MODEL_ID),
        "engine model id must be listed as an alias: {canonical_entry}"
    );

    // canonical名を指定した推論は、エンジン固有IDに戻して上流へ転送される
    let chat_resp = client
        .post(format!("http://{}/v1/chat/completions", server.addr()))
        .header("x-api-key", "sk_debug")
        .json(&json!({
            "model": CANONICAL_NAME,
            "messages": [{"role": "user", "content": CHAT_PROMPT}],
            "stream": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(chat_resp.status().as_u16(), 200);

    // 同一ホストの別プロセスがモックのポートへ要求を送りうるため、
    // 回数ではなく本テストが送った要求の内容で検証する
    let upstream_models: Vec<Value> = mock
        .received_requests()
        .await
        .expect("request recording is enabled")
        .iter()
        .filter(|req| req.method.as_str() == "POST" && req.url.path() == "/v1/chat/completions")
        .filter_map(|req| serde_json::from_slice::<Value>(&req.body).ok())
        .filter(|body| body["messages"][0]["content"] == CHAT_PROMPT)
        .map(|body| body["model"].clone())
        .collect();
    assert!(
        !upstream_models.is_empty(),
        "chat request must reach the upstream endpoint"
    );
    assert!(
        upstream_models.iter().all(|model| model == ENGINE_MODEL_ID),
        "upstream must receive the engine model id, got {upstream_models:?}"
    );
}
