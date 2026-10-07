//! 初回ヘルスチェックも通知購読後に実行されることを実バイナリで検証する。

use crate::helpers::save_notification_settings;
use llmlb::notifications::NotificationSettings;
use llmlb::types::endpoint::{Endpoint, EndpointStatus, EndpointType};
use std::fs::File;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use wiremock::{Mock, MockServer, ResponseTemplate};

struct ServerProcess(Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        // このテストが起動したプロセスだけを停止し、panic時も残さない。
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn ac3_startup_offline_arrival_reaches_notifications_without_blocking_server() {
    let directory = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite:{}", directory.path().join("llmlb.db").display());
    let pool = llmlb::bootstrap::init_db_pool(&db_url).await.unwrap();
    llmlb::db::migrations::run_migrations(&pool).await.unwrap();
    // SMTP未設定なので外部送信はない。イベントの処理は未送信理由のログで観測する。
    save_notification_settings(
        &pool,
        &NotificationSettings {
            enabled: true,
            ..NotificationSettings::default()
        },
    )
    .await;

    let upstream = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(503))
        .mount(&upstream)
        .await;
    let endpoint = Endpoint::new(
        "startup-offline".to_string(),
        upstream.uri(),
        EndpointType::OpenaiCompatible,
    );
    assert_eq!(endpoint.status, EndpointStatus::Pending);
    llmlb::db::endpoints::create_endpoint(&pool, &endpoint)
        .await
        .unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let output_path = directory.path().join("server-output.log");
    let output = File::create(&output_path).unwrap();
    let mut server = ServerProcess(
        Command::new(env!("CARGO_BIN_EXE_llmlb"))
            .args([
                "serve",
                "--no-tray",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .env("HOME", directory.path())
            .env("USERPROFILE", directory.path())
            .env("LLMLB_DATA_DIR", directory.path())
            .env("LLMLB_ADMIN_USERNAME", "startup-admin")
            .env("LLMLB_ADMIN_PASSWORD", "StartupTest123")
            .env("LLMLB_DATABASE_URL", &db_url)
            .env("LLMLB_LOG_DIR", directory.path().join("logs"))
            .env("LLMLB_LOG_LEVEL", "info")
            .env(
                "LLMLB_AUDIT_ARCHIVE_PATH",
                directory.path().join("archive.db"),
            )
            .env("LLMLB_HEALTH_CHECK_INTERVAL", "3600")
            .env_remove("LLMLB_SMTP_USERNAME")
            .env_remove("LLMLB_SMTP_PASSWORD")
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap(),
    );

    let result = tokio::time::timeout(Duration::from_secs(20), async {
        let client = reqwest::Client::new();
        loop {
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "server exited early"
            );
            let logs = std::fs::read_to_string(&output_path).unwrap();
            if logs.lines().any(|line| {
                line.contains("Offline alert was not sent; notifications are unavailable")
                    && line.contains(&endpoint.id.to_string())
            }) && client
                .get(format!("http://127.0.0.1:{port}/api/version"))
                .send()
                .await
                .is_ok_and(|response| response.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;

    let stored = llmlb::db::endpoints::get_endpoint(&pool, endpoint.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, EndpointStatus::Offline);
    assert!(
        result.is_ok(),
        "initial Offline arrival must reach notifications while the server stays available:\n{}",
        std::fs::read_to_string(output_path).unwrap()
    );
}
