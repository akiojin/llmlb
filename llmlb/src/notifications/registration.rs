//! 登録申請の管理者通知（Issue #913）。申請の保存とメール配送を独立させる。

use super::{
    Language, MailError, MailMessage, MailTransport, NotificationSnapshot, SmtpCredentials,
    SmtpMailTransport,
};
use sqlx::SqlitePool;

/// 既存の通知設定と管理者の通知先を使って申請を知らせる。
///
/// `Ok(false)` は無効・未設定を表す。パスワードやハッシュは受け取らない。
pub async fn send_registration_notification(
    pool: &SqlitePool,
    transport: &dyn MailTransport,
    credentials: &SmtpCredentials,
    username: &str,
) -> Result<bool, MailError> {
    let snapshot = NotificationSnapshot::load(pool, credentials)
        .await
        .map_err(|error| MailError::NotConfigured(error.to_string()))?;
    let delivery = match snapshot.delivery {
        Ok(delivery) => delivery,
        Err(status) => {
            tracing::debug!(?status, "Registration notification is inactive");
            return Ok(false);
        }
    };
    let (subject, body) = match delivery.language {
        Language::Ja => (
            "[llmlb] ユーザー登録申請",
            format!("ユーザー登録申請: {username}\n\nDashboard の Manage Users で申請を確認し、ロールを選択して承認するか、却下してください。承認前はログインできません。\n"),
        ),
        Language::En => (
            "[llmlb] Registration request",
            format!("Registration request: {username}\n\nReview the request in Dashboard > Manage Users. Select a role to approve, or reject the request. The applicant cannot log in before approval.\n"),
        ),
    };
    transport
        .send(&MailMessage {
            to: delivery.recipients,
            subject: subject.to_string(),
            body,
        })
        .await?;
    Ok(true)
}

/// 保存済みの申請について配送を開始する。SMTP の失敗・遅延は登録応答に影響しない。
pub fn start_registration_notification(pool: SqlitePool, username: String) {
    tokio::spawn(async move {
        let credentials = SmtpCredentials::from_env();
        let transport = SmtpMailTransport::new(pool.clone(), credentials.clone());
        if let Err(error) =
            send_registration_notification(&pool, &transport, &credentials, &username).await
        {
            tracing::warn!(%error, "Failed to send registration notification");
        }
    });
}
