//! LLM Router Server Entry Point

use clap::Parser;
use llm_router::cli::Cli;
use llm_router::config::{get_env_with_fallback_or, get_env_with_fallback_parse};
use llm_router::{api, auth, balancer, health, logging, registry, AppState};
use sqlx::sqlite::SqliteConnectOptions;
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::info;

#[derive(Clone)]
struct ServerConfig {
    host: String,
    port: u16,
}

impl ServerConfig {
    fn from_env() -> Self {
        let host = get_env_with_fallback_or("LLM_ROUTER_HOST", "ROUTER_HOST", "0.0.0.0");
        let port = get_env_with_fallback_parse("LLM_ROUTER_PORT", "ROUTER_PORT", 32768);
        Self { host, port }
    }

    fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl ServerConfig {
    fn local_host(&self) -> String {
        match self.host.as_str() {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1".to_string(),
            other => other.to_string(),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}:{}", self.local_host(), self.port)
    }

    fn dashboard_url(&self) -> String {
        format!("{}/dashboard", self.base_url())
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn main() {
    // Parse CLI for -h/--help and -V/--version support on all platforms
    match Cli::try_parse() {
        Ok(_) => {
            // No special flags, proceed with GUI tray mode
        }
        Err(e) => {
            // Handle --help and --version which clap reports as errors
            match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    // clap already printed the output, just exit
                    e.exit();
                }
                _ => {
                    // Actual error (unknown flag, etc.)
                    e.exit();
                }
            }
        }
    }

    logging::init().expect("failed to initialize logging");
    use llm_router::gui::tray::{run_with_system_tray, TrayOptions};
    use std::thread;
    use tokio::runtime::Builder;

    let config = ServerConfig::from_env();
    let tray_options = TrayOptions::new(&config.base_url(), &config.dashboard_url());

    run_with_system_tray(tray_options, move |proxy| {
        let server_config = config.clone();
        thread::spawn(move || {
            let runtime = Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build Tokio runtime for system tray mode");
            runtime.block_on(run_server(server_config));
            proxy.notify_server_exit();
        });
    });
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[tokio::main]
async fn main() {
    // Parse CLI (only -h/--help and -V/--version)
    let _cli = Cli::parse();

    // Start server
    logging::init().expect("failed to initialize logging");
    let cfg = ServerConfig::from_env();
    run_server(cfg).await;
}

async fn init_db_pool(database_url: &str) -> sqlx::Result<sqlx::SqlitePool> {
    // SQLiteファイルはディレクトリが存在しないと作成できないため、先に作成しておく
    if let Some(path) = database_url.strip_prefix("sqlite:") {
        // `sqlite::memory:` のような特殊指定はスキップ
        if !path.starts_with(':') {
            // `sqlite://` 形式に備えてスラッシュを除去し、クエリ部分を除外
            let normalized = path.trim_start_matches("//");
            let path_without_params = normalized.split('?').next().unwrap_or(normalized);
            let db_path = std::path::Path::new(path_without_params);
            if let Some(parent) = db_path.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    panic!(
                        "Failed to create database directory {}: {}",
                        parent.display(),
                        err
                    );
                }
            }
        }
    }

    let connect_options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);

    sqlx::SqlitePool::connect_with(connect_options).await
}

async fn run_server(config: ServerConfig) {
    info!("LLM Router v{}", env!("CARGO_PKG_VERSION"));

    // データベース接続プールを最初に作成（他コンポーネントが依存）
    let database_url =
        llm_router::config::get_env_with_fallback("LLM_ROUTER_DATABASE_URL", "DATABASE_URL")
            .unwrap_or_else(|| {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .expect("Failed to get home directory");
                format!("sqlite:{}/.llm-router/router.db", home)
            });

    let db_pool = init_db_pool(&database_url)
        .await
        .expect("Failed to connect to database");

    // マイグレーションを実行
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    info!("Initializing storage at ~/.llm-router/");
    let registry = registry::NodeRegistry::with_storage(db_pool.clone())
        .await
        .expect("Failed to initialize node registry");

    let load_manager = balancer::LoadManager::new(registry.clone());
    info!("Storage initialized successfully");

    let health_check_interval_secs: u64 = get_env_with_fallback_parse(
        "LLM_ROUTER_HEALTH_CHECK_INTERVAL",
        "HEALTH_CHECK_INTERVAL",
        30,
    );
    let node_timeout_secs: u64 =
        get_env_with_fallback_parse("LLM_ROUTER_NODE_TIMEOUT", "NODE_TIMEOUT", 60);

    let health_monitor = health::HealthMonitor::new(
        registry.clone(),
        health_check_interval_secs,
        node_timeout_secs,
    );
    health_monitor.start();

    // エンドポイントレジストリを初期化
    let endpoint_registry = llm_router::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to initialize endpoint registry");

    // 起動時にエンドポイントのヘルスチェックを実行
    if let Err(e) = health::run_startup_health_check(&endpoint_registry).await {
        tracing::warn!("Startup health check failed: {}", e);
    }

    // エンドポイントヘルスチェッカーをバックグラウンドで開始
    let endpoint_health_checker = health::EndpointHealthChecker::new(endpoint_registry.clone())
        .with_interval(health_check_interval_secs);
    endpoint_health_checker.start();

    let load_balancer_mode = get_env_with_fallback_or(
        "LLM_ROUTER_LOAD_BALANCER_MODE",
        "LOAD_BALANCER_MODE",
        "auto",
    );
    info!("Load balancer mode: {}", load_balancer_mode);

    // リクエスト履歴ストレージを初期化（SQLite使用）
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    if let Err(err) = request_history.import_legacy_json_if_present().await {
        tracing::warn!("Failed to import legacy request history: {}", err);
    }
    llm_router::db::request_history::start_cleanup_task(request_history.clone());

    // 管理者が存在しない場合は作成
    auth::bootstrap::ensure_admin_exists(&db_pool)
        .await
        .expect("Failed to ensure admin exists");

    // JWT秘密鍵を取得または生成（ファイル永続化対応）
    let jwt_secret = llm_router::jwt_secret::get_or_create_jwt_secret()
        .expect("Failed to get or create JWT secret");

    info!("Authentication system initialized");

    // HTTPクライアント（接続プーリング有効）を作成
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    // エンドポイントレジストリを初期化
    let endpoint_registry =
        match llm_router::registry::endpoints::EndpointRegistry::new(db_pool.clone()).await {
            Ok(reg) => {
                info!(
                    "Endpoint registry initialized with {} endpoints",
                    reg.count().await
                );
                Some(reg)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize endpoint registry: {}", e);
                None
            }
        };

    let state = AppState {
        registry: registry.clone(),
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client,
        queue_config: llm_router::config::QueueConfig::from_env(),
        event_bus: llm_router::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let router = api::create_router(state);

    let bind_addr = config.bind_addr();
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("Router server listening on {}", bind_addr);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_db_pool_creates_sqlite_file_when_missing() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("router.db");
        let db_url = format!("sqlite:{}", db_path.display());

        assert!(
            !db_path.exists(),
            "database file should not exist before initialization"
        );

        let pool = init_db_pool(&db_url)
            .await
            .expect("init_db_pool should create missing sqlite file");

        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .expect("basic query should succeed after initialization");

        assert!(
            db_path.exists(),
            "database file should be created by init_db_pool"
        );
    }
}
