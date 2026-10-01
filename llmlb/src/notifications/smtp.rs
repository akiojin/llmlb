//! SMTP トランスポート
//!
//! 接続設定は送信のたびに `settings` テーブルと環境変数由来の認証情報から組み立てる。
//! TLS は必須で、ポート 465 は接続時から TLS、それ以外は STARTTLS を要求する。

use super::config::SmtpCredentials;
use super::mailer::{MailError, MailMessage, MailTransport};
use crate::db::settings::SettingsStorage;
use async_trait::async_trait;
use sqlx::SqlitePool;

/// SMTP でメールを送信するトランスポート
pub struct SmtpMailTransport {
    settings: SettingsStorage,
    credentials: SmtpCredentials,
}

impl SmtpMailTransport {
    /// トランスポートを作成する
    pub fn new(pool: SqlitePool, credentials: SmtpCredentials) -> Self {
        Self {
            settings: SettingsStorage::new(pool),
            credentials,
        }
    }
}

#[async_trait]
impl MailTransport for SmtpMailTransport {
    async fn send(&self, message: &MailMessage) -> Result<(), MailError> {
        let _ = (message, &self.settings, &self.credentials);
        todo!("SPEC #777 T-003")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::config::{save_settings, NotificationSettings};

    fn credentials() -> SmtpCredentials {
        SmtpCredentials::new(Some("mailer".to_string()), Some("s3cret".to_string()))
    }

    fn message(to: &[&str]) -> MailMessage {
        MailMessage {
            to: to.iter().map(|address| address.to_string()).collect(),
            subject: "subject".to_string(),
            body: "body\n".to_string(),
        }
    }

    async fn pool_with(settings: NotificationSettings) -> SqlitePool {
        let pool = crate::db::test_utils::test_db_pool().await;
        save_settings(&SettingsStorage::new(pool.clone()), &settings)
            .await
            .unwrap();
        pool
    }

    fn settings_for(host: &str, port: u16) -> NotificationSettings {
        NotificationSettings {
            enabled: true,
            smtp_host: host.to_string(),
            smtp_port: port,
            smtp_from: "llmlb@example.com".to_string(),
            ..NotificationSettings::default()
        }
    }

    #[tokio::test]
    async fn send_reports_not_configured_when_smtp_settings_are_missing() {
        let pool = crate::db::test_utils::test_db_pool().await;
        let transport = SmtpMailTransport::new(pool, credentials());

        let error = transport
            .send(&message(&["ops@example.com"]))
            .await
            .unwrap_err();
        assert!(
            matches!(&error, MailError::NotConfigured(reason) if reason.contains("SMTP host")),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn send_reports_not_configured_without_credentials() {
        let pool = pool_with(settings_for("smtp.example.com", 587)).await;
        let transport = SmtpMailTransport::new(pool, SmtpCredentials::default());

        let error = transport
            .send(&message(&["ops@example.com"]))
            .await
            .unwrap_err();
        assert!(matches!(error, MailError::NotConfigured(_)), "{error:?}");
    }

    #[tokio::test]
    async fn send_rejects_bad_recipients_before_connecting() {
        // 到達不能なホスト名。宛先の検証で失敗するため接続は行われない
        let pool = pool_with(settings_for("smtp.invalid", 587)).await;
        let transport = SmtpMailTransport::new(pool, credentials());

        let error = transport
            .send(&message(&["not-an-address"]))
            .await
            .unwrap_err();
        assert!(matches!(error, MailError::InvalidAddress(_)), "{error:?}");

        let error = transport.send(&message(&[])).await.unwrap_err();
        assert!(matches!(error, MailError::Message(_)), "{error:?}");
    }

    #[tokio::test]
    async fn send_reports_delivery_failure_when_the_server_is_unreachable() {
        // 直前まで使われていたポートを閉じ、接続が拒否される宛先を作る
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let pool = pool_with(settings_for("127.0.0.1", port)).await;
        let transport = SmtpMailTransport::new(pool, credentials());

        let error = transport
            .send(&message(&["ops@example.com"]))
            .await
            .unwrap_err();
        assert!(matches!(error, MailError::Delivery(_)), "{error:?}");
        assert!(!error.to_string().contains("s3cret"), "{error}");
    }
}
