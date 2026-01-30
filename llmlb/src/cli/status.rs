//! status サブコマンド
//!
//! 起動中のサーバーの状態を表示します。

use crate::lock::{is_process_running, list_all_locks, read_lock_info};
use clap::Args;

/// status サブコマンドの引数
#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Show status of specific port only
    #[arg(short, long)]
    pub port: Option<u16>,
}

/// status コマンドを実行
pub async fn execute(args: &StatusArgs) -> Result<(), anyhow::Error> {
    if let Some(port) = args.port {
        // 特定ポートの状態を表示
        match read_lock_info(port)? {
            Some(info) => {
                if is_process_running(info.pid) {
                    println!("PORT\tPID\tSTARTED\t\t\t\tSTATUS");
                    println!("{}\t{}\t{}\tRunning", info.port, info.pid, info.started_at);
                } else {
                    println!("No active server on port {} (stale lock file found)", port);
                }
            }
            None => {
                println!("No server running on port {}", port);
            }
        }
    } else {
        // 全サーバーの状態を表示
        let locks = list_all_locks();
        if locks.is_empty() {
            println!("No servers running");
        } else {
            println!("PORT\tPID\tSTARTED\t\t\t\tSTATUS");
            for info in locks {
                println!("{}\t{}\t{}\tRunning", info.port, info.pid, info.started_at);
            }
        }
    }
    Ok(())
}
