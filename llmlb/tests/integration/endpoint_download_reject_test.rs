//! Integration Test: US8 - 非xLLMダウンロード拒否
//!
//! SPEC-66555000: エンドポイントタイプ自動判別機能
//!
//! 管理者として、非xLLMエンドポイント（Ollama/vLLM/OpenAI互換）で
//! モデルダウンロードがリクエストされた場合、エラーを返してほしい。

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::support::lb::spawn_test_lb_with_db;

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created = llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin).await;
    let admin_id = match created {
        Ok(user) => user.id,
        Err(_) => {
            llmlb::db::users::find_by_username(db_pool, "admin")
                .await
                .unwrap()
                .unwrap()
                .id
        }
    };

    let api_key = llmlb::db::api_keys::create(
        db_pool,
        "test-admin-key",
        admin_id,
        None,
        ApiKeyPermission::all(),
    )
    .await
    .expect("create admin api key");
    api_key.key
}

/// US8-拒否シナリオ1: unknownタイプのエンドポイントでダウンロード拒否
#[tokio::test]
async fn test_download_reject_unknown_type() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（オフラインなのでunknownタイプ）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Offline Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "unknown"
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
        .header("authorization", format!("Bearer {}", admin_key))
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
async fn test_download_reject_ollama_type() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // Ollamaエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Ollama Server",
            "base_url": "http://localhost:11434",  // Ollamaモック
            "endpoint_type": "ollama"
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
        .header("authorization", format!("Bearer {}", admin_key))
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
async fn test_download_reject_vllm_type() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // vLLMエンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "vLLM Server",
            "base_url": "http://localhost:8000",  // vLLMモック
            "endpoint_type": "vllm"
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
        .header("authorization", format!("Bearer {}", admin_key))
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
async fn test_download_reject_openai_compatible_type() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // OpenAI互換エンドポイント登録（モックが必要）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "OpenAI Compatible Server",
            "base_url": "http://localhost:8001",  // OpenAI互換モック
            "endpoint_type": "openai_compatible"
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
        .header("authorization", format!("Bearer {}", admin_key))
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
async fn test_download_reject_error_message() {
    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // unknownタイプのエンドポイントを登録
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": "http://localhost:9999",
            "endpoint_type": "unknown"
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
        .header("authorization", format!("Bearer {}", admin_key))
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
