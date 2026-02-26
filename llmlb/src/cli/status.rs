//! status subcommand
//!
//! Displays the current status of running servers.

use crate::lock::{is_process_running, list_all_locks, read_lock_info};
use chrono::Utc;
use clap::Args;
use reqwest::StatusCode;
use std::time::Duration;

/// Arguments for the status subcommand
#[derive(Args, Debug, Clone)]
pub struct StatusArgs {
    /// Show status of specific port only
    #[arg(short, long)]
    pub port: Option<u16>,
}

/// Execute the status command
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
                    let status_label =
                        determine_status_label(info.pid, true, info.started_at, http_status);
                    println!("PORT\tPID\tSTARTED\t\t\t\tSTATUS\tURL\tHTTP");
                    println!(
                        "{}\t{}\t{}\t{}\t{}\t{}",
                        info.port,
                        info.pid,
                        info.started_at,
                        status_label,
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
                let http_status = if info.pid == 0 || is_running {
                    check_http(&client, &dashboard_url).await
                } else {
                    None
                };
                let status_label =
                    determine_status_label(info.pid, is_running, info.started_at, http_status);
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

fn determine_status_label(
    pid: u32,
    is_running: bool,
    started_at: chrono::DateTime<chrono::Utc>,
    http_status: Option<StatusCode>,
) -> &'static str {
    if pid == 0 {
        // Windows: lock file can be held by a running process but unreadable (PID unknown).
        return if http_status.is_some() {
            "Running"
        } else {
            "Unknown"
        };
    }

    if !is_running {
        return "Stale";
    }

    if http_status.is_some() {
        return "Running";
    }

    // Process is alive but HTTP is unreachable. This commonly happens during startup before
    // the TCP listener is bound.
    let age = Utc::now() - started_at;
    if age.num_seconds() < 30 {
        "Starting"
    } else {
        "Unreachable"
    }
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
