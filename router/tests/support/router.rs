use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use llm_router::{api, balancer::LoadManager, registry::endpoints::EndpointRegistry, AppState};
use llm_router_common::auth::UserRole;
use reqwest::{Client, Response};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::http::{spawn_router, TestServer};

/// テスト用のRouterを作成する（.oneshot()スタイルのテスト用）
#[allow(dead_code)]
pub async fn create_test_router() -> (Router, SqlitePool) {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!("or-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    std::env::set_var("LLM_CONVERT_FAKE", "1");

    let db_pool = create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = test_jwt_secret();

    // EndpointRegistryを初期化
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");

    // LoadManagerはEndpointRegistryを使用
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));

    let state = AppState {
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let router = api::create_router(state);
    (router, db_pool)
}

/// テスト用のSQLiteデータベースプールを作成する
pub async fn create_test_db_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");

    // マイグレーションを実行
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// テスト用のJWT秘密鍵を生成する
pub fn test_jwt_secret() -> String {
    "test-jwt-secret-key-for-testing-only".to_string()
}

/// ルーターサーバーをテスト用に起動する
pub async fn spawn_test_router() -> TestServer {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!("or-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);
    std::env::set_var("LLM_CONVERT_FAKE", "1");

    let db_pool = create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = test_jwt_secret();

    // EndpointRegistryを初期化
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");

    // LoadManagerはEndpointRegistryを使用
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));

    let state = AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let router = api::create_router(state);
    spawn_router(router).await
}

/// 指定したルーターにノードを登録する
#[allow(dead_code)]
pub async fn register_node(
    router_addr: SocketAddr,
    node_addr: SocketAddr,
) -> reqwest::Result<Response> {
    let response = register_node_with_runtimes(router_addr, node_addr, vec![]).await?;
    Ok(response)
}

/// SPEC-66555000: POST /v0/nodes は廃止されました。
/// このヘルパー関数は後方互換性のために残されていますが、
/// 新しいテストは Endpoints API を使用してください。
///
/// 指定したルーターにノードを登録する（ランタイムタイプ指定可能）
/// レスポンスのボディには {"node_id": "...", "token": "..."} 形式が含まれます
pub async fn register_node_with_runtimes(
    router_addr: SocketAddr,
    node_addr: SocketAddr,
    supported_runtimes: Vec<&str>,
) -> reqwest::Result<Response> {
    use serde_json::json;

    // 1. 内部APIを使ってノードを登録するための仮想レスポンスを作成
    // POST /v0/nodes が廃止されたため、内部 /v0/internal/test/register-node を使用
    let payload = json!({
        "machine_name": "stub-node",
        "ip_address": node_addr.ip().to_string(),
        "runtime_version": "0.0.0-test",
        "runtime_port": node_addr.port().saturating_sub(1),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
        ],
        "supported_runtimes": supported_runtimes
    });

    // テスト専用の内部エンドポイントを使用
    Client::new()
        .post(format!(
            "http://{router_addr}/v0/internal/test/register-node"
        ))
        .header("authorization", "Bearer sk_debug")
        .json(&payload)
        .send()
        .await
}

/// 音声認識（ASR）対応エンドポイントを登録する
/// （SPEC-66555000: EndpointRegistry経由）
#[allow(dead_code)]
pub async fn register_audio_transcription_endpoint(
    router_addr: SocketAddr,
    stub_addr: SocketAddr,
) -> reqwest::Result<String> {
    register_endpoint_with_capabilities(
        router_addr,
        stub_addr,
        "Audio Transcription Endpoint",
        &["audio_transcription"],
    )
    .await
}

/// 音声合成（TTS）対応エンドポイントを登録する
/// （SPEC-66555000: EndpointRegistry経由）
#[allow(dead_code)]
pub async fn register_audio_speech_endpoint(
    router_addr: SocketAddr,
    stub_addr: SocketAddr,
) -> reqwest::Result<String> {
    register_endpoint_with_capabilities(
        router_addr,
        stub_addr,
        "Audio Speech Endpoint",
        &["audio_speech"],
    )
    .await
}

/// 画像生成対応エンドポイントを登録する
/// （SPEC-66555000: EndpointRegistry経由）
#[allow(dead_code)]
pub async fn register_image_generation_endpoint(
    router_addr: SocketAddr,
    stub_addr: SocketAddr,
) -> reqwest::Result<String> {
    register_endpoint_with_capabilities(
        router_addr,
        stub_addr,
        "Image Generation Endpoint",
        &["image_generation"],
    )
    .await
}

/// 指定したcapabilitiesでエンドポイントを登録する
/// （SPEC-66555000: EndpointRegistry経由）
#[allow(dead_code)]
pub async fn register_endpoint_with_capabilities(
    router_addr: SocketAddr,
    stub_addr: SocketAddr,
    name: &str,
    capabilities: &[&str],
) -> reqwest::Result<String> {
    let client = Client::new();

    // 1. エンドポイントを作成
    let create_response = client
        .post(format!("http://{}/v0/endpoints", router_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": format!("{} - {}", name, stub_addr),
            "base_url": format!("http://{}", stub_addr),
            "health_check_interval_secs": 30,
            "capabilities": capabilities
        }))
        .send()
        .await?;

    let create_body: Value = create_response.json().await.unwrap_or_default();
    let endpoint_id = create_body["id"].as_str().unwrap_or_default().to_string();

    // 2. エンドポイントをOnline状態にする
    let _ = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            router_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await?;

    Ok(endpoint_id)
}

/// Responses API対応エンドポイントを登録し、指定のモデルで利用可能にする
/// （SPEC-24157000: Open Responses API対応テスト用）
#[allow(dead_code)]
pub async fn register_responses_endpoint(
    router_addr: SocketAddr,
    stub_addr: SocketAddr,
    model_id: &str,
) -> reqwest::Result<String> {
    let client = Client::new();

    // 1. エンドポイントを登録
    let create_response = client
        .post(format!("http://{}/v0/endpoints", router_addr))
        .header("authorization", "Bearer sk_debug")
        .json(&json!({
            "name": format!("Responses API Test Endpoint - {}", model_id),
            "base_url": format!("http://{}", stub_addr),
            "health_check_interval_secs": 30
        }))
        .send()
        .await?;

    let create_status = create_response.status();
    let create_body: Value = create_response.json().await.unwrap_or_default();
    let endpoint_id = create_body["id"].as_str().unwrap_or_default().to_string();

    if !create_status.is_success() || endpoint_id.is_empty() {
        eprintln!(
            "Failed to create endpoint: status={}, body={}",
            create_status, create_body
        );
    }

    // 2. エンドポイントをOnline状態にする（接続テストを実行）
    let test_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/test",
            router_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await?;

    let test_status = test_response.status();
    let test_body: Value = test_response.json().await.unwrap_or_default();
    if !test_status.is_success() {
        eprintln!(
            "Failed to test endpoint: status={}, body={}",
            test_status, test_body
        );
    }

    // 3. モデルを同期
    let sync_response = client
        .post(format!(
            "http://{}/v0/endpoints/{}/sync",
            router_addr, endpoint_id
        ))
        .header("authorization", "Bearer sk_debug")
        .send()
        .await?;

    let sync_status = sync_response.status();
    let sync_body: Value = sync_response.json().await.unwrap_or_default();
    if !sync_status.is_success() {
        eprintln!(
            "Failed to sync endpoint: status={}, body={}",
            sync_status, sync_body
        );
    }

    Ok(endpoint_id)
}

/// 指定したノードを管理者として承認する
#[allow(dead_code)]
pub async fn approve_node(router_addr: SocketAddr, node_id: &str) -> reqwest::Result<Response> {
    let client = Client::new();
    let login_response = client
        .post(format!("http://{}/v0/auth/login", router_addr))
        .json(&json!({
            "username": "admin",
            "password": "test"
        }))
        .send()
        .await?;

    let login_data: Value = login_response.json().await.unwrap_or_default();
    let token = login_data["token"].as_str().unwrap_or_default();

    client
        .post(format!(
            "http://{}/v0/nodes/{}/approve",
            router_addr, node_id
        ))
        .header("authorization", format!("Bearer {}", token))
        .send()
        .await
}

/// 登録レスポンスからノードを承認し、HTTPステータスとボディを返す
#[allow(dead_code)]
pub async fn approve_node_from_register_response(
    router_addr: SocketAddr,
    register_response: Response,
) -> reqwest::Result<(reqwest::StatusCode, Value)> {
    let status = register_response.status();
    let body: Value = register_response.json().await.unwrap_or_default();

    if let Some(node_id) = body.get("node_id").and_then(|v| v.as_str()) {
        let _ = approve_node(router_addr, node_id).await?;
    }

    Ok((status, body))
}

/// テスト用の管理者ユーザーを作成してAPIキーを取得する
#[allow(dead_code)]
pub async fn create_test_api_key(router_addr: SocketAddr, db_pool: &SqlitePool) -> String {
    // 管理者ユーザーを作成
    let password_hash = llm_router::auth::password::hash_password("password123").unwrap();
    llm_router::db::users::create(db_pool, "admin", &password_hash, UserRole::Admin)
        .await
        .ok();

    let client = Client::new();

    // ログイン
    let login_response = client
        .post(format!("http://{}/v0/auth/login", router_addr))
        .json(&json!({
            "username": "admin",
            "password": "password123"
        }))
        .send()
        .await
        .expect("login should succeed");

    let login_data: Value = login_response.json().await.expect("login json");
    let jwt_token = login_data["token"].as_str().unwrap();

    // APIキーを発行
    let create_key_response = client
        .post(format!("http://{}/v0/api-keys", router_addr))
        .header("authorization", format!("Bearer {}", jwt_token))
        .json(&json!({
            "name": "Test API Key",
            "expires_at": null,
            "scopes": ["api"]
        }))
        .send()
        .await
        .expect("create api key should succeed");

    let key_data: Value = create_key_response.json().await.expect("api key json");
    key_data["key"].as_str().unwrap().to_string()
}

/// ルーターサーバーをテスト用に起動する（DBプールも返す）
#[allow(dead_code)]
pub async fn spawn_test_router_with_db() -> (TestServer, SqlitePool) {
    // テスト用に一時ディレクトリを設定
    let temp_dir = std::env::temp_dir().join(format!(
        "or-test-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::env::set_var("LLM_ROUTER_DATA_DIR", &temp_dir);

    let db_pool = create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let jwt_secret = test_jwt_secret();

    // EndpointRegistryを初期化
    let endpoint_registry = EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to create endpoint registry");

    // LoadManagerはEndpointRegistryを使用
    let load_manager = LoadManager::new(Arc::new(endpoint_registry.clone()));

    let state = AppState {
        load_manager,
        request_history,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let router = api::create_router(state);
    (spawn_router(router).await, db_pool)
}
