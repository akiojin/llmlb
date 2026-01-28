//! Integration Test: US8 - xLLMモデルダウンロード
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、xLLMエンドポイントにモデルダウンロードを
//! リクエストし、進捗を確認したい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::{lb::spawn_test_lb, xllm::spawn_mock_xllm};

/// US8-シナリオ1: xLLMエンドポイントでモデルダウンロードをリクエスト
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_xllm_model_download_request() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // xLLMタイプのエンドポイントを登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
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

    // モデルダウンロードをリクエスト
    let download_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "llama-3.2-1b"
        }))
        .send()
        .await
        .unwrap();

    // 202 Accepted を期待
    assert_eq!(download_response.status().as_u16(), 202);

    let download_body: Value = download_response.json().await.unwrap();
    assert!(download_body["task_id"].is_string());
    assert_eq!(download_body["model"], "llama-3.2-1b");
    assert_eq!(download_body["status"], "pending");
}

/// US8-シナリオ2: ダウンロード進捗を確認
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T124で実装後に有効化"]
async fn test_xllm_model_download_progress() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // エンドポイント登録
    let register_response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
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

    // ダウンロードリクエスト
    let _ = client
        .post(format!(
            "http://{}/v0/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "llama-3.2-1b"
        }))
        .send()
        .await
        .unwrap();

    // 進捗を確認
    let progress_response = client
        .get(format!(
            "http://{}/v0/endpoints/{}/download/progress",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    assert_eq!(progress_response.status().as_u16(), 200);

    let progress_body: Value = progress_response.json().await.unwrap();
    let tasks = progress_body["tasks"].as_array().unwrap();

    // ダウンロードタスクが存在することを確認
    assert!(!tasks.is_empty(), "Should have at least one download task");

    // タスクの構造を検証
    let task = &tasks[0];
    assert!(task["task_id"].is_string());
    assert!(task["model"].is_string());
    assert!(task["status"].is_string());
    assert!(task["progress"].is_number());
}

/// US8-シナリオ3: 複数モデルの同時ダウンロード
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123-T124で実装後に有効化"]
async fn test_xllm_model_download_multiple() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // エンドポイント登録
    let register_response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
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

    // 複数モデルをダウンロードリクエスト
    for model in ["llama-3.2-1b", "llama-3.2-3b", "mistral-7b"] {
        let _ = client
            .post(format!(
                "http://{}/v0/endpoints/{}/download",
                server.addr(),
                endpoint_id
            ))
            .header("authorization", "Bearer sk_debug")
            .json(&json!({
                "model": model
            }))
            .send()
            .await
            .unwrap();
    }

    // 進捗一覧を確認
    let progress_response = client
        .get(format!(
            "http://{}/v0/endpoints/{}/download/progress",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await
        .unwrap();

    let progress_body: Value = progress_response.json().await.unwrap();
    let tasks = progress_body["tasks"].as_array().unwrap();

    assert_eq!(tasks.len(), 3, "Should have 3 download tasks");
}

/// US8-シナリオ4: ダウンロード完了後、モデル一覧に反映
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - 全タスク実装後に有効化"]
async fn test_xllm_model_download_completion() {
    let server = spawn_test_lb().await;
    let xllm = spawn_mock_xllm().await;
    let client = Client::new();

    // エンドポイント登録
    let register_response = client
        .post(format!("http://{}/v0/endpoints", server.addr()))
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

    // モデルダウンロードリクエスト
    let _ = client
        .post(format!(
            "http://{}/v0/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "llama-3.2-1b"
        }))
        .send()
        .await
        .unwrap();

    // TODO: ダウンロード完了を待つ
    // TODO: モデル同期を実行
    // TODO: モデル一覧にllama-3.2-1bが含まれることを確認
}
