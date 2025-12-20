use std::net::SocketAddr;

use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::auth::UserRole;
use reqwest::{Client, Response};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::http::{spawn_router, TestServer};

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

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let jwt_secret = test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    let router = api::create_router(state);
    spawn_router(router).await
}

/// 指定したルーターにノードを登録する
pub async fn register_node(
    router_addr: SocketAddr,
    node_addr: SocketAddr,
) -> reqwest::Result<Response> {
    let response = register_node_with_runtimes(router_addr, node_addr, vec![]).await?;
    Ok(response)
}

/// 指定したルーターにノードを登録する（ランタイムタイプ指定可能）
pub async fn register_node_with_runtimes(
    router_addr: SocketAddr,
    node_addr: SocketAddr,
    _supported_runtimes: Vec<&str>,
) -> reqwest::Result<Response> {
    let payload = json!({
        "machine_name": "stub-node",
        "ip_address": node_addr.ip().to_string(),
        "runtime_version": "0.0.0-test",
        // テストスタブはHTTPポートで直接実行される。
        // ルーターのヘルスチェックは runtime_port + 1 でアクセスするため、
        // テストスタブのポートに対して -1 を計算して渡す。
        // (例：スタブが port 12345 で実行 → runtime_port: 12344 を登録
        //      → ルーターが 12344 + 1 = 12345 でヘルスチェック)
        "runtime_port": node_addr.port().saturating_sub(1),
        "gpu_available": true,
        "gpu_devices": [
            {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
        ]
    });

    // supported_runtimesが指定されている場合はペイロードに追加
    let payload = if !_supported_runtimes.is_empty() {
        let mut p = payload;
        p["supported_runtimes"] = json!(_supported_runtimes);
        p
    } else {
        payload
    };

    Client::new()
        .post(format!("http://{router_addr}/v0/nodes"))
        .json(&payload)
        .send()
        .await
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
            "expires_at": null
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

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = create_test_db_pool().await;
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1);
    let jwt_secret = test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    let router = api::create_router(state);
    (spawn_router(router).await, db_pool)
}
