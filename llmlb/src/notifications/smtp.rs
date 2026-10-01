//! SMTP トランスポート
//!
//! 接続設定は送信のたびに `settings` テーブルと環境変数由来の認証情報から組み立てる。
//! TLS は必須で、ポート 465 は接続時から TLS、それ以外は STARTTLS を要求する。

use super::config::{load_settings, smtp_config, SmtpConfig, SmtpCredentials, IMPLICIT_TLS_PORT};
use super::mailer::{MailError, MailMessage, MailTransport};
use crate::db::settings::SettingsStorage;
use async_trait::async_trait;
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::SqlitePool;
use std::time::Duration;

/// SMTP サーバーとのやり取り 1 回あたりのタイムアウト
const SMTP_TIMEOUT: Duration = Duration::from_secs(30);

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
        let loaded = load_settings(&self.settings)
            .await
            .map_err(|error| MailError::NotConfigured(error.to_string()))?;
        let config = smtp_config(&loaded, &self.credentials).map_err(MailError::NotConfigured)?;
        let email = build_message(&config, message)?;

        let builder = if config.port == IMPLICIT_TLS_PORT {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
        }
        .map_err(|error| MailError::Delivery(error.to_string()))?;
        let mailer = builder
            .port(config.port)
            .credentials(Credentials::new(config.username, config.password))
            .timeout(Some(SMTP_TIMEOUT))
            .build();

        mailer
            .send(email)
            .await
            .map(|_| ())
            .map_err(|error| MailError::Delivery(error.to_string()))
    }
}

fn mailbox(address: &str) -> Result<Mailbox, MailError> {
    address
        .trim()
        .parse::<Address>()
        .map(|address| Mailbox::new(None, address))
        .map_err(|_| MailError::InvalidAddress(address.trim().to_string()))
}

/// プレーンテキストのメールを組み立てる
fn build_message(config: &SmtpConfig, message: &MailMessage) -> Result<Message, MailError> {
    if message.to.is_empty() {
        return Err(MailError::Message("no recipients".to_string()));
    }
    let mut builder = Message::builder()
        .from(mailbox(&config.from)?)
        .subject(message.subject.as_str());
    for recipient in &message.to {
        builder = builder.to(mailbox(recipient)?);
    }
    builder
        .header(ContentType::TEXT_PLAIN)
        .body(message.body.clone())
        .map_err(|error| MailError::Message(error.to_string()))
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

    fn smtp() -> SmtpConfig {
        SmtpConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            from: "llmlb@example.com".to_string(),
            username: "mailer".to_string(),
            password: "s3cret".to_string(),
        }
    }

    #[test]
    fn build_message_sets_sender_recipients_subject_and_plain_text_body() {
        let mail = MailMessage {
            to: vec!["a@example.com".to_string(), "b@example.com".to_string()],
            subject: "[llmlb] 日次ダイジェスト 2026-10-01".to_string(),
            body: "状態一覧:\n- [Online] gpu-a\n".to_string(),
        };

        let email = build_message(&smtp(), &mail).unwrap();

        let envelope = email.envelope();
        assert_eq!(
            envelope.from().map(|address| address.to_string()),
            Some("llmlb@example.com".to_string())
        );
        let recipients: Vec<String> = envelope
            .to()
            .iter()
            .map(|address| address.to_string())
            .collect();
        assert_eq!(recipients, vec!["a@example.com", "b@example.com"]);

        let raw = String::from_utf8(email.formatted()).unwrap();
        assert!(raw.contains("From: llmlb@example.com"), "{raw}");
        assert!(raw.contains("To: a@example.com, b@example.com"), "{raw}");
        assert!(
            raw.contains("Content-Type: text/plain; charset=utf-8"),
            "{raw}"
        );
        // 認証情報はメール本体に現れない
        assert!(!raw.contains("s3cret"), "{raw}");
        assert!(!raw.contains("mailer"), "{raw}");
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
