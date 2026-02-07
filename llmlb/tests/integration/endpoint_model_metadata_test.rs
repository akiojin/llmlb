//! Integration Test: US9 - モデルメタデータ取得
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、xLLM/Ollamaエンドポイントからモデルの
//! メタデータ（最大トークン数/コンテキスト長）を取得したい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::{lb::spawn_test_lb, ollama::spawn_mock_ollama, xllm::spawn_mock_xllm};

/// US9-シナリオ1: xLLMエンドポイントからモデルメタデータを取得
#[tokio::test]
#[ignore = "メタデータAPI未実装 - T125, T129で実装後に有効化"]
async fn test_xllm_model_metadata_retrieval() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // xLLMエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "xLLM Server",
            "base_url": format!("http://{}", xllm.addr())
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // モデル同期を実行してモデルを登録
    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(sync_response.status().as_u16(), 200);

    // モデルメタデータ取得
    let metadata_response = client
        .get(format!(
            "http://{}/api/endpoints/{}/models/llama3:8b/info",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(metadata_response.status().as_u16(), 200);

    let metadata: Value = metadata_response.json().await.unwrap();

    // 期待されるフィールド
    assert!(metadata["model_id"].is_string());
    assert!(metadata["endpoint_id"].is_string());

    // max_tokensフィールド（xLLMの場合）
    if metadata.get("max_tokens").is_some() {
        assert!(
            metadata["max_tokens"].is_number() || metadata["max_tokens"].is_null(),
            "max_tokens should be a number or null"
        );
    }
}

/// US9-シナリオ2: Ollamaエンドポイントからモデルメタデータを取得
#[tokio::test]
#[ignore = "メタデータAPI未実装 - T125, T130で実装後に有効化"]
async fn test_ollama_model_metadata_retrieval() {
    let server = spawn_test_lb().await;
    let ollama = spawn_mock_ollama().await;
    let client = Client::new();

    // Ollamaエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Ollama Server",
            "base_url": format!("http://{}", ollama.addr())
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // モデル同期を実行してモデルを登録
    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(sync_response.status().as_u16(), 200);

    // モデルメタデータ取得
    let metadata_response = client
        .get(format!(
            "http://{}/api/endpoints/{}/models/llama3:8b/info",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(metadata_response.status().as_u16(), 200);

    let metadata: Value = metadata_response.json().await.unwrap();

    // max_tokensフィールド（Ollamaの場合はnum_ctxから取得）
    if metadata.get("max_tokens").is_some() {
        assert!(
            metadata["max_tokens"].is_number() || metadata["max_tokens"].is_null(),
            "max_tokens should be a number or null"
        );
    }
}

/// US9-シナリオ3: モデル同期時にmax_tokensが自動取得される
#[tokio::test]
#[ignore = "メタデータAPI未実装 - T133で実装後に有効化"]
async fn test_model_sync_retrieves_max_tokens() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // xLLMエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "xLLM Server",
            "base_url": format!("http://{}", xllm.addr())
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // モデル同期を実行
    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(sync_response.status().as_u16(), 200);

    // モデル一覧を取得してmax_tokensが含まれることを確認
    let models_response = client
        .get(format!(
            "http://{}/api/endpoints/{}/models",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    if models_response.status().as_u16() == 200 {
        let models_body: Value = models_response.json().await.unwrap();
        if let Some(models) = models_body["models"].as_array() {
            for model in models {
                // max_tokensフィールドが存在する（null許容）
                assert!(
                    model.get("max_tokens").is_some(),
                    "Model should have max_tokens field"
                );
            }
        }
    }
}

/// US9-シナリオ4: vLLM/OpenAI互換ではメタデータ取得非サポート
#[tokio::test]
#[ignore = "メタデータAPI未実装 - T125で実装後に有効化"]
async fn test_vllm_metadata_not_supported() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // vLLMエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "vLLM Server",
            "base_url": "http://localhost:8000"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // メタデータ取得リクエスト
    let metadata_response = client
        .get(format!(
            "http://{}/api/endpoints/{}/models/some-model/info",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // 400 Bad Request または 404 Not Foundを期待
    assert!(
        metadata_response.status().as_u16() == 400 || metadata_response.status().as_u16() == 404,
        "vLLM should not support metadata retrieval"
    );
}

/// US9-シナリオ5: 存在しないモデルのメタデータ取得
#[tokio::test]
#[ignore = "メタデータAPI未実装 - T125で実装後に有効化"]
async fn test_nonexistent_model_metadata() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // エンドポイント登録
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Server",
            "base_url": format!("http://{}", xllm.addr())
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // モデル同期を実行してモデルを登録
    let sync_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/sync",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();
    assert_eq!(sync_response.status().as_u16(), 200);

    // 存在しないモデルのメタデータ取得
    let metadata_response = client
        .get(format!(
            "http://{}/api/endpoints/{}/models/nonexistent-model-12345/info",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    // 404 Not Foundを期待
    assert_eq!(metadata_response.status().as_u16(), 404);
}
