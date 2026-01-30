//! stop サブコマンド
//!
//! 起動中のサーバーを停止します。

use crate::lock::{is_process_running, lock_path, read_lock_info, stop_process};
use clap::Args;
use std::time::Duration;

/// stop サブコマンドの引数
#[derive(Args, Debug, Clone)]
pub struct StopArgs {
    /// Port of the server to stop
    #[arg(short, long)]
    pub port: u16,

    /// Timeout in seconds to wait for server to stop
    #[arg(short, long, default_value = "5")]
    pub timeout: u64,
}

/// stop コマンドを実行
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
