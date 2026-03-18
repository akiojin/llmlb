//! Integration Test: US8 - ダウンロード対応/非対応エンドポイント
//!
//! SPEC-e8e9326e: エンドポイントタイプ自動判別機能
//!
//! ダウンロード対応: xLLM, Ollama, LM Studio
//! ダウンロード非対応: vLLM, OpenAI互換 → エラーを返す

use llmlb::common::auth::{ApiKeyPermission, UserRole};
use reqwest::Client;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::lb::spawn_test_lb_with_db;

async fn create_admin_api_key(db_pool: &SqlitePool) -> String {
    let password_hash = llmlb::auth::password::hash_password("password123").unwrap();
    let created =
        llmlb::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin, false).await;
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

/// Ollamaとして検出されるモックサーバーを作成するヘルパー
async fn create_ollama_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{"name": "llama3:8b", "size": 4000000000_i64}]
        })))
        .mount(&mock)
        .await;
    mock
}

/// vLLMとして検出されるモックサーバーを作成するヘルパー
async fn create_vllm_mock() -> MockServer {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("server", "vLLM/0.4.0")
                .set_body_json(json!({
                    "object": "list",
                    "data": [{"id": "test-model", "object": "model", "owned_by": "vllm"}]
                })),
        )
        .mount(&mock)
        .await;
    mock
}

/// US8-拒否シナリオ1: ダウンロード非対応エンドポイント（OpenAI互換）
/// エンドポイントはOpenAI互換として自動検出される
#[tokio::test]
async fn test_download_reject_non_xllm_type() {
    let mock = create_openai_compatible_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // エンドポイント登録（自動検出でopenai_compatibleとして登録される）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Offline Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
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

    // 400 Bad Requestを期待（OpenAI互換はダウンロード非対応）
    assert_eq!(download_response.status().as_u16(), 400);

    let error_body: Value = download_response.json().await.unwrap();
    let error_msg = error_body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("not supported") || error_msg.contains("download"),
        "Error message should indicate download is not supported: {}",
        error_msg
    );
}

/// US8-受理シナリオ: Ollamaタイプのエンドポイントでダウンロード受理
#[tokio::test]
async fn test_download_accept_ollama_type() {
    let mock = create_ollama_mock().await;

    // Ollama pull APIのモックを追加
    Mock::given(method("POST"))
        .and(path("/api/pull"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success"
        })))
        .mount(&mock)
        .await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // Ollamaエンドポイント登録（自動検出でollamaとして登録される）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Ollama Server",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
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

    // 202 Accepted を期待（Ollamaはダウンロード対応）
    assert_eq!(download_response.status().as_u16(), 202);
}

/// US8-拒否シナリオ3: vLLMタイプのエンドポイントでダウンロード拒否
#[tokio::test]
async fn test_download_reject_vllm_type() {
    let mock = create_vllm_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // vLLMエンドポイント登録（自動検出でvllmとして登録される）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "vLLM Server",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
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
    let mock = create_openai_compatible_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // OpenAI互換エンドポイント登録（自動検出でopenai_compatibleとして登録される）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "OpenAI Compatible Server",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
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
    let mock = create_openai_compatible_mock().await;

    let (server, db_pool) = spawn_test_lb_with_db().await;
    let client = Client::new();
    let admin_key = create_admin_api_key(&db_pool).await;

    // 非xLLMエンドポイントを登録（自動検出でopenai_compatibleとして登録される）
    let register_response = client
        .post(format!("http://{}/api/endpoints", server.addr()))
        .header("authorization", format!("Bearer {}", admin_key))
        .json(&json!({
            "name": "Test Endpoint",
            "base_url": mock.uri()
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(register_response.status().as_u16(), 201);
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
    let error_msg = error_body["error"].as_str().unwrap_or("");
    assert!(
        error_msg.contains("not supported") || error_msg.contains("download"),
        "Error message should explain download is not supported: {}",
        error_msg
    );
}
