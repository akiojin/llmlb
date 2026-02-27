//! stop subcommand
//!
//! Stops a running server.

use crate::lock::{is_process_running, lock_path, read_lock_info, stop_process};
use clap::Args;
use std::time::Duration;

/// Arguments for the stop subcommand
#[derive(Args, Debug, Clone)]
pub struct StopArgs {
    /// Port of the server to stop
    #[arg(short, long)]
    pub port: u16,

    /// Timeout in seconds to wait for server to stop
    #[arg(short, long, default_value = "5")]
    pub timeout: u64,
}

/// Execute the stop command
pub async fn execute(args: &StopArgs) -> Result<(), anyhow::Error> {
    let port = args.port;

    // ロック情報を読み取り
    let lock_info = match read_lock_info(port)? {
        Some(info) => info,
        None => {
            println!("No server running on port {}", port);
            return Ok(());
        }
    };

    // PIDが存在するか確認
    if !is_process_running(lock_info.pid) {
        // ロックファイルは存在するがプロセスは存在しない（残留ロック）
        let path = lock_path(port);
        std::fs::remove_file(&path)?;
        println!(
            "Warning: Stale lock file found (PID {} not running), cleaned up",
            lock_info.pid
        );
        return Ok(());
    }

    // プロセスを停止
    println!(
        "Stopping server on port {} (PID: {})...",
        port, lock_info.pid
    );
    stop_process(lock_info.pid)?;

    // 終了を待機
    let timeout = Duration::from_secs(args.timeout);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if !is_process_running(lock_info.pid) {
            println!("Server stopped successfully");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // タイムアウト
    println!(
        "Warning: Server did not stop within {} seconds. You may need to kill it manually: kill -9 {}",
        args.timeout, lock_info.pid
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn unique_test_port() -> u16 {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("failed to reserve test port");
        let port = listener
            .local_addr()
            .expect("failed to read test port")
            .port();
        drop(listener);
        port
    }

    #[tokio::test]
    async fn execute_succeeds_when_lock_file_is_missing() {
        let args = StopArgs {
            port: unique_test_port(),
            timeout: 1,
        };
        execute(&args)
            .await
            .expect("stop should succeed when no lock exists");
    }

    #[tokio::test]
    async fn execute_removes_stale_lock_file() {
        let port = unique_test_port();
        let path = crate::lock::lock_path(port);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create lock dir");
        }
        let stale = crate::lock::LockInfo {
            pid: u32::MAX,
            started_at: Utc::now(),
            port,
        };
        std::fs::write(
            &path,
            serde_json::to_string(&stale).expect("failed to serialize lock info"),
        )
        .expect("failed to write lock file");

        let args = StopArgs { port, timeout: 1 };
        execute(&args)
            .await
            .expect("stop should clean stale lock and return Ok");
        assert!(!path.exists(), "stale lock file should be removed");
    }
}
