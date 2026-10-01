//! 運用通知（SPEC #777）
//!
//! エンドポイントの稼働状況をメールで運用者へ届ける。送信はトランスポート抽象
//! （[`MailTransport`]）の背後に置き、SMTP の秘密情報は環境変数、非秘密設定は
//! `settings` テーブルから読む。設定が未設定・不正な場合は通知だけを無効化し、
//! アプリ本体の起動は妨げない。

pub mod config;
pub mod daily_digest;
pub mod digest;
pub mod mailer;
pub mod schedule;
pub mod smtp;
pub mod template;

pub use config::{
    load_settings, resolve_delivery, save_settings, smtp_config, Delivery, InvalidSetting,
    LoadedSettings, NotificationSettings, NotificationState, NotificationStatus, SmtpConfig,
    SmtpCredentials,
};
pub use daily_digest::{start_daily_digest_task, DailyDigestScheduler, DigestOutcome, DigestTick};
pub use digest::{DigestReport, EndpointDigestEntry};
pub use mailer::{parse_address, MailError, MailMessage, MailTransport};
pub use smtp::SmtpMailTransport;
pub use template::{render_daily_digest, Language, RenderedMail};
