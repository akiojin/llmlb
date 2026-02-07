//! status サブコマンド
//!
//! 起動中のサーバーの状態を表示します。

use crate::lock::{is_process_running, list_all_locks, read_lock_info};
use clap::Args;
use reqwest::StatusCode;
use std::time::Duration;

/// status サブコマンドの引数
#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Show status of specific port only
    #[arg(short, long)]
    pub port: Option<u16>,
}

/// status コマンドを実行
pub async fn execute(args: &StatusArgs) -> Result<(), anyhow::Error> {
    let client = reqwest::Client::new();
    if let Some(port) = args.port {
        // 特定ポートの状態を表示
        match read_lock_info(port)? {
            Some(info) => {
                let base_url = format!("http://127.0.0.1:{}", info.port);
                let dashboard_url = format!("{}/dashboard/", base_url);
                if is_process_running(info.pid) {
                    let http_status = check_http(&client, &dashboard_url).await;
                    println!("PORT\tPID\tSTARTED\t\t\t\tSTATUS\tURL\tHTTP");
                    println!(
                        "{}\t{}\t{}\tRunning\t{}\t{}",
                        info.port,
                        info.pid,
                        info.started_at,
                        dashboard_url,
                        format_http_status(http_status)
                    );
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
            println!("PORT\tPID\tSTARTED\t\t\t\tSTATUS\tURL\tHTTP");
            for info in locks {
                let base_url = format!("http://127.0.0.1:{}", info.port);
                let dashboard_url = format!("{}/dashboard/", base_url);
                let is_running = info.pid != 0 && is_process_running(info.pid);
                let status_label = if info.pid == 0 {
                    "Unknown"
                } else if is_running {
                    "Running"
                } else {
                    "Stale"
                };
                let http_status = if is_running {
                    check_http(&client, &dashboard_url).await
                } else {
                    None
                };
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    info.port,
                    info.pid,
                    info.started_at,
                    status_label,
                    dashboard_url,
                    format_http_status(http_status)
                );
            }
        }
    }
    Ok(())
}

async fn check_http(client: &reqwest::Client, url: &str) -> Option<StatusCode> {
    client
        .get(url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|resp| resp.status())
        .ok()
}

fn format_http_status(status: Option<StatusCode>) -> String {
    match status {
        Some(code) if code.is_success() => "OK".to_string(),
        Some(code) => format!("HTTP {}", code),
        None => "UNREACHABLE".to_string(),
    }
}
