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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

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

    #[test]
    fn determine_status_label_covers_all_branches() {
        let now = Utc::now();

        assert_eq!(
            determine_status_label(0, false, now, Some(StatusCode::OK)),
            "Running"
        );
        assert_eq!(determine_status_label(0, false, now, None), "Unknown");
        assert_eq!(
            determine_status_label(12345, false, now, Some(StatusCode::OK)),
            "Stale"
        );
        assert_eq!(
            determine_status_label(12345, true, now, Some(StatusCode::OK)),
            "Running"
        );
        assert_eq!(
            determine_status_label(12345, true, now - ChronoDuration::seconds(10), None),
            "Starting"
        );
        assert_eq!(
            determine_status_label(12345, true, now - ChronoDuration::seconds(90), None),
            "Unreachable"
        );
    }

    #[test]
    fn format_http_status_covers_variants() {
        assert_eq!(format_http_status(Some(StatusCode::OK)), "OK");
        assert_eq!(
            format_http_status(Some(StatusCode::SERVICE_UNAVAILABLE)),
            "HTTP 503 Service Unavailable"
        );
        assert_eq!(format_http_status(None), "UNREACHABLE");
    }

    #[tokio::test]
    async fn check_http_returns_status_when_reachable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/dashboard/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let status = check_http(&client, &format!("{}/dashboard/", server.uri())).await;
        assert_eq!(status, Some(StatusCode::OK));
    }

    #[tokio::test]
    async fn check_http_returns_none_when_unreachable() {
        let client = reqwest::Client::new();
        let status = check_http(&client, "http://127.0.0.1:9/dashboard/").await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn execute_succeeds_when_port_has_no_lock_file() {
        let args = StatusArgs {
            port: Some(unique_test_port()),
        };
        execute(&args)
            .await
            .expect("status command should succeed for missing lock");
    }

    #[tokio::test]
    async fn execute_succeeds_for_stale_lock() {
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

        let args = StatusArgs { port: Some(port) };
        let result = execute(&args).await;
        let _ = std::fs::remove_file(&path);

        result.expect("status command should handle stale lock");
    }
}
