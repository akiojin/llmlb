use std::net::SocketAddr;

use or_router::{
    api, balancer::LoadManager, registry::NodeRegistry, tasks::DownloadTaskManager, AppState,
};
use reqwest::{Client, Response};
use serde_json::json;
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
    std::env::set_var("OLLAMA_ROUTER_DATA_DIR", &temp_dir);

    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let request_history =
        std::sync::Arc::new(or_router::db::request_history::RequestHistoryStorage::new().unwrap());
    let task_manager = DownloadTaskManager::new();
    let db_pool = create_test_db_pool().await;
    let jwt_secret = test_jwt_secret();

    let state = AppState {
        registry,
        load_manager,
        request_history,
        task_manager,
        db_pool,
        jwt_secret,
    };

    let router = api::create_router(state);
    spawn_router(router).await
}

/// 指定したルーターにノードを登録する
pub async fn register_node(
    router_addr: SocketAddr,
    node_addr: SocketAddr,
) -> reqwest::Result<Response> {
    Client::new()
        .post(format!("http://{router_addr}/api/nodes"))
        .json(&json!({
            "machine_name": "stub-node",
            "ip_address": node_addr.ip().to_string(),
            "ollama_version": "0.0.0-test",
            // ノードAPIポートは ollama_port+1 という前提のため、APIポートから1引いた値を報告する
            "ollama_port": node_addr.port().saturating_sub(1),
            "gpu_available": true,
            "gpu_devices": [
                {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
            ]
        }))
        .send()
        .await
}
