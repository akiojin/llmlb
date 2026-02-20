//! LLM load balancer Server Entry Point

use clap::Parser;
use llmlb::cli::{Cli, Commands};
use llmlb::config::{get_env_with_fallback_or, get_env_with_fallback_parse};
use llmlb::lock::ServerLock;
use llmlb::{api, auth, balancer, health, logging, AppState};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::Row;
use std::net::SocketAddr;
use std::str::FromStr;
use tracing::{info, warn};

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
        Some(Commands::Internal(args)) => {
            logging::init().expect("failed to initialize logging");
            if let Err(e) = llmlb::cli::internal::execute(args.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
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
        Some(Commands::Assistant(args)) => {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            if let Err(e) = runtime.block_on(llmlb::cli::assistant::execute(&args.command)) {
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
            if args.no_tray {
                let runtime = Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build Tokio runtime for server mode");
                runtime.block_on(run_server(config, None));
                return;
            }
            let tray_options = TrayOptions::new(&config.base_url(), &config.dashboard_url());
            let fallback_config = config.clone();

            let result = run_with_system_tray(tray_options, move |proxy| {
                let server_config = config.clone();
                thread::spawn(move || {
                    let runtime = Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("failed to build Tokio runtime for system tray mode");
                    runtime.block_on(run_server(server_config, Some(proxy.clone())));
                    proxy.notify_server_exit();
                });
            });
            if let Err(err) = result {
                tracing::error!("System tray initialization failed: {err}");
                let runtime = Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build Tokio runtime for server mode");
                runtime.block_on(run_server(fallback_config, None));
            }
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
    let fallback_config = config.clone();

    let result = run_with_system_tray(tray_options, move |proxy| {
        let server_config = config.clone();
        thread::spawn(move || {
            let runtime = Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to build Tokio runtime for system tray mode");
            runtime.block_on(run_server(server_config, Some(proxy.clone())));
            proxy.notify_server_exit();
        });
    });
    if let Err(err) = result {
        tracing::error!("System tray initialization failed: {err}");
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build Tokio runtime for server mode");
        runtime.block_on(run_server(fallback_config, None));
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Handle subcommands
    match cli.command {
        Some(Commands::Internal(args)) => {
            logging::init().expect("failed to initialize logging");
            if let Err(e) = llmlb::cli::internal::execute(args.command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            return;
        }
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
        Some(Commands::Assistant(args)) => {
            if let Err(e) = llmlb::cli::assistant::execute(&args.command).await {
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

// Migration 005: pre-v4.4.0 checksum (original file before any SPEC renames)
const MIGRATION_005_OLD_CHECKSUM: [u8; 48] = [
    0xbb, 0x58, 0x31, 0x50, 0x93, 0xaf, 0x8c, 0xc7, 0x44, 0xed, 0x00, 0xf7, 0xdd, 0xe3, 0xc4, 0xd5,
    0xd2, 0xca, 0xdb, 0xf4, 0xa8, 0x92, 0x20, 0x0e, 0x4f, 0x39, 0xbf, 0xdf, 0xd3, 0x34, 0x61, 0xfa,
    0x3e, 0x7f, 0x72, 0xeb, 0x9a, 0xd3, 0x33, 0xc6, 0x05, 0xb2, 0xc3, 0xe7, 0x78, 0xd0, 0x2d, 0xee,
];
// Migration 005: v4.4.0 checksum (after first SPEC rename in v4.5.0 plan)
const MIGRATION_005_V440_CHECKSUM: [u8; 48] = [
    0x5b, 0x77, 0x47, 0x63, 0xce, 0xd7, 0xd8, 0xbc, 0x14, 0xe9, 0x6b, 0x88, 0x1c, 0x33, 0x90, 0x73,
    0x5a, 0xe9, 0x92, 0x74, 0x46, 0xbd, 0x0e, 0x82, 0xc4, 0x2a, 0xe5, 0xe5, 0x8d, 0x0b, 0xcf, 0x50,
    0x43, 0xb4, 0xbf, 0x00, 0xa2, 0x8e, 0x3a, 0x95, 0x89, 0xa8, 0x1c, 0x08, 0x9c, 0x26, 0xcc, 0xa0,
];
// Migration 005: current file checksum
const MIGRATION_005_NEW_CHECKSUM: [u8; 48] = [
    0x0f, 0xa6, 0x82, 0x71, 0xef, 0x76, 0x91, 0xb0, 0x57, 0x9d, 0xcb, 0x19, 0x4e, 0x01, 0x99, 0x89,
    0x78, 0xf8, 0xdf, 0x1d, 0x4b, 0x21, 0x5c, 0xce, 0x18, 0xb1, 0x26, 0x1b, 0x38, 0x57, 0x60, 0x35,
    0x0c, 0x13, 0x32, 0xa3, 0xd8, 0x3c, 0xc8, 0x54, 0x1a, 0x84, 0xa0, 0x0b, 0x0d, 0xea, 0x65, 0xa5,
];
// Migration 006: v4.4.0/v4.5.0 checksum → current file checksum
const MIGRATION_006_OLD_CHECKSUM: [u8; 48] = [
    0x75, 0x40, 0x0a, 0xfd, 0x5b, 0xb7, 0x64, 0x14, 0xab, 0xc1, 0x00, 0x00, 0x6f, 0x5b, 0x53, 0xb0,
    0x17, 0xdc, 0xe0, 0x93, 0xd9, 0x00, 0xc8, 0xf2, 0x63, 0x01, 0x4a, 0x4b, 0xe8, 0xd6, 0xc5, 0x2b,
    0x39, 0x8a, 0xbd, 0xb4, 0xc3, 0x5e, 0xad, 0xf9, 0xbb, 0x14, 0xfa, 0xbd, 0xfb, 0x68, 0x63, 0x96,
];
const MIGRATION_006_NEW_CHECKSUM: [u8; 48] = [
    0xd1, 0x49, 0xbf, 0xea, 0xbc, 0x07, 0x3e, 0x9e, 0x1d, 0x6c, 0xd5, 0xda, 0x10, 0x12, 0x9c, 0xca,
    0x54, 0xf8, 0xa4, 0x67, 0xf4, 0xe8, 0xc9, 0x6c, 0x9f, 0xce, 0x43, 0xb5, 0x0a, 0x7d, 0xfb, 0xc9,
    0x7c, 0xf7, 0x35, 0xd4, 0xba, 0xff, 0xef, 0x10, 0x61, 0x1b, 0xb3, 0xb5, 0xfd, 0x76, 0xd3, 0x33,
];
// Migration 007: v4.4.0 checksum → current file checksum
const MIGRATION_007_OLD_CHECKSUM: [u8; 48] = [
    0xbc, 0x4c, 0xa0, 0x47, 0x18, 0x85, 0xa3, 0xd3, 0x1d, 0x6c, 0x91, 0x58, 0x5f, 0x76, 0x9f, 0xd8,
    0x79, 0xf6, 0xcb, 0x0a, 0x8c, 0xe2, 0x66, 0xc6, 0x05, 0xd5, 0xb1, 0x3d, 0xc7, 0x8c, 0x2f, 0x9e,
    0xa4, 0x32, 0x7e, 0x30, 0x94, 0x7b, 0x13, 0x15, 0x06, 0x38, 0x81, 0x55, 0x8b, 0xca, 0x0f, 0xa7,
];
const MIGRATION_007_NEW_CHECKSUM: [u8; 48] = [
    0x94, 0xc9, 0x05, 0x28, 0xc7, 0xb9, 0x96, 0xef, 0xb9, 0x22, 0x22, 0xa4, 0x46, 0xa2, 0x68, 0xae,
    0xce, 0xe8, 0x62, 0xc9, 0xc1, 0xd5, 0x50, 0x0b, 0x50, 0x37, 0xcf, 0x2d, 0xf3, 0x19, 0xe9, 0xa5,
    0xdb, 0x1f, 0x65, 0xd9, 0x1e, 0xc3, 0x45, 0x2a, 0xe8, 0x63, 0xa9, 0x2a, 0x8f, 0x6e, 0xd4, 0x6b,
];
const MIGRATION_008_OLD_CHECKSUM: [u8; 48] = [
    0x40, 0xc9, 0xe6, 0x46, 0x26, 0x8e, 0xa3, 0xfb, 0xe8, 0x0b, 0xd5, 0x99, 0x7d, 0xa8, 0x94, 0x44,
    0x41, 0x49, 0x7d, 0x42, 0x06, 0xc1, 0xa9, 0x45, 0xd5, 0x97, 0xdc, 0x16, 0x32, 0x35, 0x9d, 0x1d,
    0x3b, 0x18, 0x72, 0xb3, 0x1a, 0x10, 0xbb, 0x6b, 0x9a, 0x7f, 0xcb, 0x32, 0x97, 0x9a, 0x74, 0xa7,
];
const MIGRATION_008_NEW_CHECKSUM: [u8; 48] = [
    0x09, 0x7a, 0xe1, 0x69, 0x3f, 0x87, 0x81, 0x8a, 0x35, 0x46, 0x94, 0x54, 0x35, 0xfc, 0xfc, 0x96,
    0xae, 0xd4, 0x00, 0xb7, 0xdb, 0x44, 0x3f, 0x7c, 0xf7, 0x8a, 0xd8, 0xb4, 0x72, 0xc0, 0x56, 0xf5,
    0x67, 0x2d, 0x7a, 0xbb, 0xd4, 0x38, 0xba, 0x86, 0xdf, 0x5b, 0xd4, 0xec, 0xa1, 0x23, 0x70, 0x05,
];
// Migration 010: v4.4.0 checksum → current file checksum
const MIGRATION_010_OLD_CHECKSUM: [u8; 48] = [
    0x45, 0xeb, 0x00, 0x87, 0x16, 0x2b, 0x72, 0x68, 0x49, 0xd1, 0xf9, 0x13, 0xa1, 0xef, 0x90, 0x20,
    0x37, 0x70, 0xe2, 0xb5, 0xac, 0xca, 0xd9, 0x95, 0x6b, 0x27, 0x59, 0x18, 0xf1, 0x8c, 0x99, 0xe7,
    0x84, 0xbe, 0xc3, 0x23, 0xe7, 0x7b, 0xa9, 0xd9, 0x83, 0x21, 0xf0, 0x79, 0xe0, 0x96, 0xb9, 0x0f,
];
const MIGRATION_010_NEW_CHECKSUM: [u8; 48] = [
    0x1e, 0x74, 0xfb, 0x8f, 0x5b, 0x52, 0xc5, 0x5b, 0x04, 0x27, 0xf6, 0xc1, 0xc2, 0x3a, 0x96, 0x19,
    0x23, 0x28, 0x96, 0x3f, 0xa2, 0x3f, 0x80, 0x92, 0x47, 0x13, 0x97, 0x6b, 0xbb, 0x94, 0xcd, 0xb7,
    0xe5, 0x0c, 0x42, 0x8d, 0xb4, 0x3e, 0xad, 0x10, 0xa0, 0x5a, 0x80, 0x09, 0xea, 0x3a, 0x40, 0x2d,
];
// Migration 011: v4.4.0 checksum → current file checksum
const MIGRATION_011_OLD_CHECKSUM: [u8; 48] = [
    0xa6, 0xce, 0x7e, 0xf6, 0x6b, 0xad, 0xaa, 0x37, 0x9c, 0x28, 0x41, 0xe4, 0x60, 0x10, 0xf9, 0x75,
    0x17, 0x52, 0x78, 0xe0, 0x5f, 0x76, 0x27, 0xe3, 0xd0, 0x15, 0x77, 0x1a, 0x50, 0xa4, 0x06, 0xcb,
    0x60, 0xc7, 0x1e, 0x41, 0xfb, 0xdc, 0xb4, 0xb3, 0x2f, 0x44, 0x4f, 0x35, 0x3b, 0x6a, 0x5a, 0xa0,
];
const MIGRATION_011_NEW_CHECKSUM: [u8; 48] = [
    0x09, 0x01, 0x56, 0x3f, 0x6c, 0x4a, 0xa9, 0xc0, 0xae, 0x72, 0x4e, 0xc3, 0x08, 0xf4, 0xf4, 0xa4,
    0x87, 0x83, 0x23, 0x56, 0x6b, 0xe3, 0x31, 0x9b, 0x05, 0x1d, 0xf6, 0xab, 0xfa, 0x38, 0x94, 0xe0,
    0xd9, 0xf1, 0x52, 0xb7, 0xef, 0x1a, 0x3c, 0x6d, 0x7a, 0xa4, 0x0d, 0xe2, 0x33, 0xe0, 0x42, 0xc0,
];
// Migration 014: v4.4.0/v4.5.0 checksum → current file checksum
const MIGRATION_014_OLD_CHECKSUM: [u8; 48] = [
    0xf6, 0xeb, 0x48, 0x0a, 0x08, 0xcb, 0xc5, 0x2f, 0x59, 0x8b, 0xd8, 0xa8, 0x80, 0x58, 0x3a, 0x8d,
    0x68, 0x2e, 0x6f, 0x44, 0xe5, 0x62, 0x27, 0x59, 0x40, 0x02, 0x06, 0x43, 0xa6, 0x2b, 0xa2, 0xdd,
    0x8f, 0xd6, 0xb7, 0x60, 0xc8, 0x85, 0x08, 0x84, 0x54, 0x74, 0xee, 0xa0, 0x2a, 0xc9, 0xae, 0x47,
];
const MIGRATION_014_NEW_CHECKSUM: [u8; 48] = [
    0x5f, 0xcb, 0x39, 0x8f, 0x23, 0x62, 0x44, 0xbe, 0x93, 0x6c, 0xc7, 0x3a, 0x29, 0x4b, 0x6b, 0xc7,
    0xe2, 0x37, 0x29, 0xe8, 0xe9, 0xf8, 0x01, 0xb1, 0xdb, 0xfe, 0x95, 0x56, 0xa2, 0x9b, 0xb5, 0xcc,
    0xe7, 0x1b, 0x49, 0x92, 0xc7, 0x1b, 0x3f, 0x94, 0x91, 0xb0, 0xd8, 0x00, 0x82, 0xdd, 0xe8, 0x0f,
];

struct MigrationChecksumOverride {
    version: i64,
    old: &'static [u8; 48],
    new: &'static [u8; 48],
}

const MIGRATION_CHECKSUM_OVERRIDES: &[MigrationChecksumOverride] = &[
    // 005: pre-v4.4.0 → current
    MigrationChecksumOverride {
        version: 5,
        old: &MIGRATION_005_OLD_CHECKSUM,
        new: &MIGRATION_005_NEW_CHECKSUM,
    },
    // 005: v4.4.0 → current
    MigrationChecksumOverride {
        version: 5,
        old: &MIGRATION_005_V440_CHECKSUM,
        new: &MIGRATION_005_NEW_CHECKSUM,
    },
    // 006: v4.4.0/v4.5.0 → current
    MigrationChecksumOverride {
        version: 6,
        old: &MIGRATION_006_OLD_CHECKSUM,
        new: &MIGRATION_006_NEW_CHECKSUM,
    },
    // 007: v4.4.0 → current
    MigrationChecksumOverride {
        version: 7,
        old: &MIGRATION_007_OLD_CHECKSUM,
        new: &MIGRATION_007_NEW_CHECKSUM,
    },
    // 008: existing
    MigrationChecksumOverride {
        version: 8,
        old: &MIGRATION_008_OLD_CHECKSUM,
        new: &MIGRATION_008_NEW_CHECKSUM,
    },
    // 010: v4.4.0 → current
    MigrationChecksumOverride {
        version: 10,
        old: &MIGRATION_010_OLD_CHECKSUM,
        new: &MIGRATION_010_NEW_CHECKSUM,
    },
    // 011: v4.4.0 → current
    MigrationChecksumOverride {
        version: 11,
        old: &MIGRATION_011_OLD_CHECKSUM,
        new: &MIGRATION_011_NEW_CHECKSUM,
    },
    // 014: v4.4.0/v4.5.0 → current
    MigrationChecksumOverride {
        version: 14,
        old: &MIGRATION_014_OLD_CHECKSUM,
        new: &MIGRATION_014_NEW_CHECKSUM,
    },
];

async fn reconcile_migration_checksums(pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    if row.0 == 0 {
        return Ok(());
    }

    for override_entry in MIGRATION_CHECKSUM_OVERRIDES {
        let checksum_row = sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(override_entry.version)
            .fetch_optional(pool)
            .await?;
        let Some(checksum_row) = checksum_row else {
            continue;
        };

        let checksum: Vec<u8> = checksum_row.try_get("checksum")?;
        if checksum == override_entry.old.as_slice() {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                .bind(override_entry.new.as_slice())
                .bind(override_entry.version)
                .execute(pool)
                .await?;
            info!(
                "Updated migration checksum for version {} to latest format",
                override_entry.version
            );
        }
    }

    Ok(())
}

fn maybe_raise_nofile_limit() {
    #[cfg(unix)]
    {
        use std::cmp::min;

        // macOS では launchd 起動時に open files 上限が 256 など低く設定されることがあり、
        // 受け付け不能 (EMFILE) や SQLite の open 失敗 (SQLITE_CANTOPEN) につながる。
        const DESIRED_NOFILE: libc::rlim_t = 65_536;

        unsafe {
            let mut current = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut current) != 0 {
                tracing::debug!(
                    "getrlimit(RLIMIT_NOFILE) failed: {}",
                    std::io::Error::last_os_error()
                );
                return;
            }

            let target = min(DESIRED_NOFILE, current.rlim_max);
            if current.rlim_cur >= target {
                return;
            }

            let updated = libc::rlimit {
                rlim_cur: target,
                rlim_max: current.rlim_max,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &updated) != 0 {
                tracing::warn!(
                    "Failed to raise RLIMIT_NOFILE from {} to {}: {}",
                    current.rlim_cur,
                    target,
                    std::io::Error::last_os_error()
                );
                return;
            }

            tracing::info!(
                "Raised RLIMIT_NOFILE from {} to {}",
                current.rlim_cur,
                target
            );
        }
    }
}

/// サーバー起動時に全エンドポイントのタイプを再検出する
///
/// 前回起動時から変更されている可能性があるため、登録済みの全エンドポイントに対して
/// タイプ検出を実行し、変更があれば更新する。
/// 検出失敗時は既存設定を保持し、ヘルスチェックでの再評価に委ねる。
async fn redetect_all_endpoints(
    registry: &llmlb::registry::endpoints::EndpointRegistry,
    http_client: &reqwest::Client,
) {
    use llmlb::detection::detect_endpoint_type_with_client;
    use tracing::warn;

    const STARTUP_REDETECTION_TIMEOUT_SECS: u64 = 10;

    let endpoints = registry.list().await;
    let total = endpoints.len();

    if total == 0 {
        info!("No endpoints registered; skipping startup re-detection");
        return;
    }

    info!(
        total = total,
        "Starting endpoint type re-detection on server startup"
    );

    let mut failed: usize = 0;
    let mut updated: usize = 0;

    for ep in &endpoints {
        match tokio::time::timeout(
            std::time::Duration::from_secs(STARTUP_REDETECTION_TIMEOUT_SECS),
            detect_endpoint_type_with_client(http_client, &ep.base_url, ep.api_key.as_deref()),
        )
        .await
        {
            Ok(Ok(result)) => {
                if result.endpoint_type == ep.endpoint_type {
                    continue;
                }
                if registry
                    .update_endpoint_type(ep.id, result.endpoint_type)
                    .await
                    .is_ok()
                {
                    info!(
                        endpoint_id = %ep.id,
                        name = %ep.name,
                        old_type = ?ep.endpoint_type,
                        new_type = ?result.endpoint_type,
                        "Endpoint type changed during re-detection"
                    );
                    updated += 1;
                }
            }
            Ok(Err(err)) => {
                warn!(
                    endpoint_id = %ep.id,
                    name = %ep.name,
                    error = %err,
                    "Endpoint type re-detection failed on startup; keeping existing configuration"
                );
                failed += 1;
            }
            Err(_) => {
                warn!(
                    endpoint_id = %ep.id,
                    name = %ep.name,
                    timeout_secs = STARTUP_REDETECTION_TIMEOUT_SECS,
                    "Endpoint type re-detection timed out on startup; keeping existing configuration"
                );
                failed += 1;
            }
        }
    }

    info!(total, failed, updated, "Endpoint re-detection complete");
}

async fn run_server(
    config: ServerConfig,
    #[cfg(any(target_os = "windows", target_os = "macos"))] tray_proxy: Option<
        llmlb::gui::tray::TrayEventProxy,
    >,
) {
    info!("LLM Load Balancer v{}", env!("CARGO_PKG_VERSION"));
    maybe_raise_nofile_limit();

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

    reconcile_migration_checksums(&db_pool)
        .await
        .expect("Failed to reconcile database migrations");

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

    // HTTPクライアント（接続プーリング有効）を作成
    // NOTE: re-detection やヘルスチェック等で使用するため、早期に作成する
    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    // サーバー起動時のエンドポイントタイプ再検出
    redetect_all_endpoints(&endpoint_registry, &http_client).await;

    let health_check_interval_secs: u64 =
        get_env_with_fallback_parse("LLMLB_HEALTH_CHECK_INTERVAL", "HEALTH_CHECK_INTERVAL", 30);

    // エンドポイントヘルスチェッカーをバックグラウンドで開始
    // NOTE: 起動時の並列ヘルスチェックもこのstart()内で実行し、サーバー起動をブロックしない。
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
    llmlb::db::endpoint_daily_stats::start_daily_stats_task(db_pool.clone());

    // 管理者が存在しない場合は作成
    auth::bootstrap::ensure_admin_exists(&db_pool)
        .await
        .expect("Failed to ensure admin exists");

    // JWT秘密鍵を取得または生成（ファイル永続化対応）
    let jwt_secret =
        llmlb::jwt_secret::get_or_create_jwt_secret().expect("Failed to get or create JWT secret");

    info!("Authentication system initialized");

    // Self-update components
    let inference_gate = llmlb::inference_gate::InferenceGate::default();
    let shutdown = llmlb::shutdown::ShutdownController::default();
    let update_manager = llmlb::update::UpdateManager::new(
        http_client.clone(),
        inference_gate.clone(),
        shutdown.clone(),
    )
    .expect("Failed to initialize update manager");

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let mgr = update_manager.clone();
        llmlb::gui::tray::set_update_apply_handler(move || mgr.request_apply());
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    if let Some(proxy) = tray_proxy {
        update_manager.set_tray_proxy(proxy).await;
    }
    update_manager.start_background_tasks();

    info!(
        "Endpoint registry initialized with {} endpoints",
        endpoint_registry.count().await
    );

    // 監査ログシステムの初期化 (SPEC-8301d106)
    let audit_log_storage =
        std::sync::Arc::new(llmlb::db::audit_log::AuditLogStorage::new(db_pool.clone()));
    let audit_log_writer = llmlb::audit::writer::AuditLogWriter::new(
        llmlb::db::audit_log::AuditLogStorage::new(db_pool.clone()),
        llmlb::audit::writer::AuditLogWriterConfig::default(),
    );
    info!("Audit log system initialized");

    // 起動時ハッシュチェーン検証 (SPEC-8301d106)
    {
        let storage_ref = &*audit_log_storage;
        match llmlb::audit::hash_chain::verify_chain(storage_ref).await {
            Ok(result) => {
                if result.valid {
                    info!(
                        batches_checked = result.batches_checked,
                        "Audit log hash chain verification passed"
                    );
                } else {
                    warn!(
                        tampered_batch = ?result.tampered_batch,
                        message = ?result.message,
                        "Audit log hash chain verification FAILED - tampering detected"
                    );
                }
            }
            Err(e) => {
                warn!("Audit log hash chain verification error: {}", e);
            }
        }
    }

    // 24時間ごとの定期ハッシュチェーン検証タスク (SPEC-8301d106)
    {
        let periodic_storage = audit_log_storage.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            // 最初のtickはスキップ（起動時検証は上で実施済み）
            interval.tick().await;
            loop {
                interval.tick().await;
                match llmlb::audit::hash_chain::verify_chain(&periodic_storage).await {
                    Ok(result) => {
                        if result.valid {
                            info!(
                                batches_checked = result.batches_checked,
                                "Periodic audit log hash chain verification passed"
                            );
                        } else {
                            warn!(
                                tampered_batch = ?result.tampered_batch,
                                message = ?result.message,
                                "Periodic audit log hash chain verification FAILED"
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Periodic audit log hash chain verification error: {}", e);
                    }
                }
            }
        });
    }

    // アーカイブDBプールの初期化 (SPEC-8301d106)
    let archive_path = std::env::var("LLMLB_AUDIT_ARCHIVE_PATH").unwrap_or_else(|_| {
        let db_path =
            std::env::var("LLMLB_DB_PATH").unwrap_or_else(|_| "load_balancer.db".to_string());
        let parent = std::path::Path::new(&db_path)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        parent
            .join("audit_archive.db")
            .to_string_lossy()
            .to_string()
    });
    let audit_archive_pool = match llmlb::db::audit_log::create_archive_pool(&archive_path).await {
        Ok(pool) => {
            info!(path = %archive_path, "Audit archive DB initialized");
            Some(pool)
        }
        Err(e) => {
            warn!("Failed to initialize audit archive DB: {}", e);
            None
        }
    };

    // 24時間ごとの定期アーカイブタスク (SPEC-8301d106)
    if let Some(ref archive_pool) = audit_archive_pool {
        let archive_storage = audit_log_storage.clone();
        let archive_pool_clone = archive_pool.clone();
        let retention_days: i64 = std::env::var("LLMLB_AUDIT_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
            interval.tick().await; // 最初のtickをスキップ
            loop {
                interval.tick().await;
                match archive_storage
                    .archive_old_entries(retention_days, &archive_pool_clone)
                    .await
                {
                    Ok(count) => {
                        if count > 0 {
                            info!(count, retention_days, "Archived old audit log entries");
                        }
                    }
                    Err(e) => {
                        warn!("Audit log archive task error: {}", e);
                    }
                }
            }
        });
    }

    let state = AppState {
        load_manager,
        request_history,
        db_pool,
        jwt_secret,
        http_client,
        queue_config: llmlb::config::QueueConfig::from_env(),
        event_bus: llmlb::events::create_shared_event_bus(),
        endpoint_registry,
        inference_gate,
        shutdown: shutdown.clone(),
        update_manager,
        audit_log_writer,
        audit_log_storage,
        audit_archive_pool,
    };

    let app = api::create_app(state);

    let bind_addr = config.bind_addr();

    // グレースフルシャットダウン用のシグナルハンドリング
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("LLM Load Balancer server listening on {}", bind_addr);

    // シグナルハンドリングを設定
    let shutdown_signal = shutdown_signal(shutdown);

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
async fn shutdown_signal(shutdown: llmlb::shutdown::ShutdownController) {
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
        _ = shutdown.wait() => {
            info!("Shutdown requested, shutting down...");
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

    #[tokio::test]
    async fn reconcile_migration_checksums_updates_old_checksums() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("load balancer.db");
        let db_url = format!("sqlite:{}", db_path.display());
        let pool = init_db_pool(&db_url)
            .await
            .expect("init_db_pool should create missing sqlite file");

        sqlx::query(
            r#"
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
);
            "#,
        )
        .execute(&pool)
        .await
        .expect("should create _sqlx_migrations table");

        // Insert old checksums for all affected migrations (simulates pre-v4.4.0 DB)
        let insert_sql = "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?1, ?2, ?3, ?4, ?5)";
        let old_entries: &[(i64, &[u8; 48])] = &[
            (5, &MIGRATION_005_OLD_CHECKSUM),
            (6, &MIGRATION_006_OLD_CHECKSUM),
            (7, &MIGRATION_007_OLD_CHECKSUM),
            (8, &MIGRATION_008_OLD_CHECKSUM),
            (10, &MIGRATION_010_OLD_CHECKSUM),
            (11, &MIGRATION_011_OLD_CHECKSUM),
            (14, &MIGRATION_014_OLD_CHECKSUM),
        ];
        for (version, checksum) in old_entries {
            sqlx::query(insert_sql)
                .bind(*version)
                .bind("test")
                .bind(true)
                .bind(checksum.as_slice())
                .bind(0_i64)
                .execute(&pool)
                .await
                .unwrap_or_else(|_| panic!("should insert migration row for version {version}"));
        }

        reconcile_migration_checksums(&pool)
            .await
            .expect("reconcile should succeed");

        // Verify all checksums were updated to new values
        let expected: &[(i64, &[u8; 48])] = &[
            (5, &MIGRATION_005_NEW_CHECKSUM),
            (6, &MIGRATION_006_NEW_CHECKSUM),
            (7, &MIGRATION_007_NEW_CHECKSUM),
            (8, &MIGRATION_008_NEW_CHECKSUM),
            (10, &MIGRATION_010_NEW_CHECKSUM),
            (11, &MIGRATION_011_NEW_CHECKSUM),
            (14, &MIGRATION_014_NEW_CHECKSUM),
        ];
        for (version, expected_checksum) in expected {
            let row = sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
                .bind(*version)
                .fetch_one(&pool)
                .await
                .unwrap_or_else(|_| panic!("should read checksum for version {version}"));
            let checksum: Vec<u8> = row
                .try_get("checksum")
                .unwrap_or_else(|_| panic!("should get checksum for version {version}"));
            assert_eq!(
                checksum,
                expected_checksum.as_slice(),
                "checksum mismatch for migration version {version}"
            );
        }
    }

    #[tokio::test]
    async fn reconcile_migration_checksums_handles_v440_to_current() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("load balancer.db");
        let db_url = format!("sqlite:{}", db_path.display());
        let pool = init_db_pool(&db_url)
            .await
            .expect("init_db_pool should create missing sqlite file");

        sqlx::query(
            r#"
CREATE TABLE IF NOT EXISTS _sqlx_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    success BOOLEAN NOT NULL,
    checksum BLOB NOT NULL,
    execution_time BIGINT NOT NULL
);
            "#,
        )
        .execute(&pool)
        .await
        .expect("should create _sqlx_migrations table");

        // Insert v4.4.0 checksum for migration 005 (intermediate state)
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(5_i64)
        .bind("test")
        .bind(true)
        .bind(MIGRATION_005_V440_CHECKSUM.as_slice())
        .bind(0_i64)
        .execute(&pool)
        .await
        .expect("should insert migration row");

        reconcile_migration_checksums(&pool)
            .await
            .expect("reconcile should succeed");

        let row = sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = 5")
            .fetch_one(&pool)
            .await
            .expect("should read checksum");
        let checksum: Vec<u8> = row.try_get("checksum").expect("should get checksum");

        assert_eq!(
            checksum, MIGRATION_005_NEW_CHECKSUM,
            "v4.4.0 checksum should be reconciled to current"
        );
    }
}
