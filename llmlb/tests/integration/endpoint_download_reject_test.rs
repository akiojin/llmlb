//! Integration Test: US8 - 非xLLMダウンロード拒否
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、非xLLMエンドポイント（Ollama/vLLM/OpenAI互換）で
//! モデルダウンロードがリクエストされた場合、エラーを返してほしい。

use reqwest::Client;
use serde_json::{json, Value};

use crate::support::lb::spawn_test_lb;

/// US8-拒否シナリオ1: unknownタイプのエンドポイントでダウンロード拒否
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_download_reject_unknown_type() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // エンドポイント登録（オフラインなのでunknownタイプ）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Offline Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // ダウンロードリクエスト
    let download_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/download",
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

    // 400 Bad Requestを期待
    assert_eq!(download_response.status().as_u16(), 400);

    let error_body: Value = download_response.json().await.unwrap();
    assert!(
        error_body["error"].as_str().unwrap_or("").contains("xLLM"),
        "Error message should mention xLLM requirement"
    );
}

/// US8-拒否シナリオ2: Ollamaタイプのエンドポイントでダウンロード拒否
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_download_reject_ollama_type() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // Ollamaエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Ollama Server",
            "base_url": "http://localhost:11434"  // Ollamaモック
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // ダウンロードリクエスト
    let download_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "llama3:8b"
        }))
        .send()
        .await
        .unwrap();

    // 400 Bad Requestを期待
    assert_eq!(download_response.status().as_u16(), 400);
}

/// US8-拒否シナリオ3: vLLMタイプのエンドポイントでダウンロード拒否
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_download_reject_vllm_type() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // vLLMエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "vLLM Server",
            "base_url": "http://localhost:8000"  // vLLMモック
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // ダウンロードリクエスト
    let download_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "some-model"
        }))
        .send()
        .await
        .unwrap();

    // 400 Bad Requestを期待
    assert_eq!(download_response.status().as_u16(), 400);
}

/// US8-拒否シナリオ4: OpenAI互換タイプのエンドポイントでダウンロード拒否
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_download_reject_openai_compatible_type() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // OpenAI互換エンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "OpenAI Compatible Server",
            "base_url": "http://localhost:8001"  // OpenAI互換モック
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // ダウンロードリクエスト
    let download_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/download",
            server.addr(),
            endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "model": "gpt-3.5-turbo"
        }))
        .send()
        .await
        .unwrap();

    // 400 Bad Requestを期待
    assert_eq!(download_response.status().as_u16(), 400);
}

/// US8-拒否シナリオ5: エラーメッセージの検証
#[tokio::test]
#[ignore = "ダウンロードAPI未実装 - T123で実装後に有効化"]
async fn test_download_reject_error_message() {
    let server = spawn_test_lb().await;
    let client = Client::new();

    // unknownタイプのエンドポイントを登録
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999"
        }))
        .send()
        .await
        .unwrap();

    let body: Value = register_response.json().await.unwrap();
    let endpoint_id = body["id"].as_str().unwrap();

    // ダウンロードリクエスト
    let download_response = client
        .post(format!(
            "http://{}/api/endpoints/{}/download",
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

    assert_eq!(download_response.status().as_u16(), 400);

    let error_body: Value = download_response.json().await.unwrap();

    // エラーメッセージの内容を検証
    // "Model download is only supported for xLLM endpoints"
    let error_msg = error_body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("xLLM") || error_msg.contains("download"),
        "Error message should explain download is xLLM-only: {}",
        error_msg
    );
}
