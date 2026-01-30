//! LLM load balancer Server Entry Point

use clap::Parser;
use llmlb::cli::{Cli, Commands};
use llmlb::config::{get_env_with_fallback_or, get_env_with_fallback_parse};
use llmlb::lock::ServerLock;
use llmlb::{api, auth, balancer, health, logging, AppState};
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
        let host = get_env_with_fallback_or("LLMLB_HOST", "LLMLB_HOST", "0.0.0.0");
        let port = get_env_with_fallback_parse("LLMLB_PORT", "LLMLB_PORT", 32768);
        Self { host, port }
    }

    fn from_args(host: String, port: u16) -> Self {
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
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.exit();
            }
            _ => {
                e.exit();
            }
        },
    };

    // Handle subcommands
    match cli.command {
        Some(Commands::Stop(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::stop::execute(&args)) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Status(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::status::execute(&args)) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Serve(args)) => {
            // Fall through to GUI mode with specified port
            logging::init().expect("failed to initialize logging");
            use llmlb::gui::tray::{run_with_system_tray, TrayOptions};
            use std::thread;
            use tokio::runtime::Builder;

            let config = ServerConfig::from_args(args.host, args.port);
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
            return;
        }
        None => {
            // No subcommand - use default GUI mode
        }
    }

    logging::init().expect("failed to initialize logging");
    use llmlb::gui::tray::{run_with_system_tray, TrayOptions};
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
    let cli = Cli::parse();

    // Handle subcommands
    match cli.command {
        Some(Commands::Stop(args)) => {
            if let Err(e) = llmlb::cli::stop::execute(&args).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Status(args)) => {
            if let Err(e) = llmlb::cli::status::execute(&args).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
        Some(Commands::Serve(args)) => {
            logging::init().expect("failed to initialize logging");
            let cfg = ServerConfig::from_args(args.host, args.port);
            run_server(cfg).await;
            return;
        }
        None => {
            // No subcommand - default to serve
            logging::init().expect("failed to initialize logging");
            let cfg = ServerConfig::from_env();
            run_server(cfg).await;
        }
    }
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
    info!("LLM Load Balancer v{}", env!("CARGO_PKG_VERSION"));

    // シングル実行制約: ロックを取得
    let _server_lock = match ServerLock::acquire(config.port) {
        Ok(lock) => {
            info!(
                "Lock acquired for port {} (PID: {})",
                config.port,
                lock.info().pid
            );
            lock
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // データベース接続プールを最初に作成（他コンポーネントが依存）
    let database_url = llmlb::config::get_env_with_fallback("LLMLB_DATABASE_URL", "DATABASE_URL")
        .unwrap_or_else(|| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .expect("Failed to get home directory");
            format!("sqlite:{}/.llmlb/load balancer.db", home)
        });

    let db_pool = init_db_pool(&database_url)
        .await
        .expect("Failed to connect to database");

    // マイグレーションを実行
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    info!("Initializing storage at ~/.llmlb/");

    // エンドポイントレジストリを初期化（新アーキテクチャ）
    let endpoint_registry = llmlb::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to initialize endpoint registry");
    let endpoint_registry_arc = std::sync::Arc::new(endpoint_registry.clone());

    // LoadManagerをEndpointRegistryで初期化
    let load_manager = balancer::LoadManager::new(endpoint_registry_arc.clone());
    info!("Storage initialized successfully");

    let health_check_interval_secs: u64 =
        get_env_with_fallback_parse("LLMLB_HEALTH_CHECK_INTERVAL", "HEALTH_CHECK_INTERVAL", 30);

    // 起動時にエンドポイントのヘルスチェックを実行
    if let Err(e) = health::run_startup_health_check(&endpoint_registry).await {
        tracing::warn!("Startup health check failed: {}", e);
    }

    // エンドポイントヘルスチェッカーをバックグラウンドで開始
    let endpoint_health_checker = health::EndpointHealthChecker::new(endpoint_registry.clone())
        .with_interval(health_check_interval_secs);
    endpoint_health_checker.start();

    let load_balancer_mode =
        get_env_with_fallback_or("LLMLB_LOAD_BALANCER_MODE", "LOAD_BALANCER_MODE", "auto");
    info!("Load balancer mode: {}", load_balancer_mode);

    // リクエスト履歴ストレージを初期化（SQLite使用）
    let request_history = std::sync::Arc::new(
        llmlb::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    if let Err(err) = request_history.import_legacy_json_if_present().await {
        tracing::warn!("Failed to import legacy request history: {}", err);
    }
    llmlb::db::request_history::start_cleanup_task(request_history.clone());

    // 管理者が存在しない場合は作成
    auth::bootstrap::ensure_admin_exists(&db_pool)
        .await
        .expect("Failed to ensure admin exists");

    // JWT秘密鍵を取得または生成（ファイル永続化対応）
    let jwt_secret =
        llmlb::jwt_secret::get_or_create_jwt_secret().expect("Failed to get or create JWT secret");

    info!("Authentication system initialized");

    // HTTPクライアント（接続プーリング有効）を作成
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    // エンドポイントレジストリを初期化
    let endpoint_registry = llmlb::registry::endpoints::EndpointRegistry::new(db_pool.clone())
        .await
        .expect("Failed to initialize endpoint registry");
    info!(
        "Endpoint registry initialized with {} endpoints",
        endpoint_registry.count().await
    );

    let state = AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client,
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
    };

    let app = api::create_app(state);

    let bind_addr = config.bind_addr();

    // グレースフルシャットダウン用のシグナルハンドリング
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("LLM Load Balancer server listening on {}", bind_addr);

    // シグナルハンドリングを設定
    let shutdown_signal = shutdown_signal();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
    .await
    .expect("Server error");

    info!("Server shutdown complete");
    // _server_lock はここでDropされ、ロックが解除される
}

/// シャットダウンシグナルを待機
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = terminate => {
            info!("Received SIGTERM, shutting down...");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_db_pool_creates_sqlite_file_when_missing() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("load balancer.db");
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
