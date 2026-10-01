//! 日次ダイジェストのスケジューラ
//!
//! 既存の定期処理（`tokio::time::interval`）はプロセス起動基準で、再起動すると実行時刻が
//! ずれる。日次ダイジェストは壁時計基準で次回実行時刻を求めて待機し、最終送信日を
//! `settings` に永続化することで、再起動をまたいでも時刻がずれず同日に二重送信しない。

use super::config::{
    Delivery, NotificationSnapshot, NotificationState, NotificationStatus, SmtpCredentials,
    KEY_DAILY_DIGEST_LAST_SENT_DATE,
};
use super::digest::DigestReport;
use super::mailer::{MailMessage, MailTransport};
use super::schedule::next_run;
use super::smtp::SmtpMailTransport;
use super::template::render_daily_digest;
use crate::common::error::{LbError, RouterResult};
use crate::db::settings::SettingsStorage;
use chrono::{Duration, NaiveDate, NaiveDateTime};
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{info, warn};

/// 待機の上限。設定の変更や時計の補正を、この間隔以内に反映する
const POLL_INTERVAL_SECS: i64 = 60;
/// 送信に失敗したあと、再試行するまでの間隔
const RETRY_INTERVAL_SECS: i64 = 15 * 60;
/// 最終送信日の保存形式
const DATE_FORMAT: &str = "%Y-%m-%d";

/// 1 回の判定結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestOutcome {
    /// 送信した
    Sent {
        /// 宛先アドレス
        recipients: Vec<String>,
    },
    /// まだ送信時刻ではない、または当日分を送信済み
    NotDue,
    /// 通知が無効、または設定の未設定・不正で送信できない
    Inactive(NotificationStatus),
    /// 送信に失敗した（後で再試行する）
    Failed(String),
}

/// 1 回の判定結果と、次に判定すべき時刻
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestTick {
    /// 判定結果
    pub outcome: DigestOutcome,
    /// 次に判定すべき時刻（サーバーのローカル時刻）
    pub next_wake: NaiveDateTime,
}

/// 日次ダイジェストのスケジューラ
pub struct DailyDigestScheduler {
    pool: SqlitePool,
    settings: SettingsStorage,
    transport: Arc<dyn MailTransport>,
    credentials: SmtpCredentials,
    /// 直近にログへ出した状態（変化したときだけログに出す）
    last_status: Option<NotificationStatus>,
    /// 送信失敗後、この時刻までは再試行しない
    retry_not_before: Option<NaiveDateTime>,
    /// このプロセスが最後に送信した日（最終送信日の永続化に失敗しても二重送信しない）
    sent_on: Option<NaiveDate>,
}

impl DailyDigestScheduler {
    /// スケジューラを作成する
    pub fn new(
        pool: SqlitePool,
        transport: Arc<dyn MailTransport>,
        credentials: SmtpCredentials,
    ) -> Self {
        Self {
            settings: SettingsStorage::new(pool.clone()),
            pool,
            transport,
            credentials,
            last_status: None,
            retry_not_before: None,
            sent_on: None,
        }
    }

    /// `now`（サーバーのローカル時刻）時点で送信要否を判定し、必要なら送信する
    pub async fn tick(&mut self, now: NaiveDateTime) -> DigestTick {
        let poll = now + Duration::seconds(POLL_INTERVAL_SECS);

        let (delivery, last_sent) = match self.read_state(now.date()).await {
            Ok((Ok(delivery), last_sent)) => (delivery, last_sent),
            Ok((Err(status), _)) => {
                self.log_status_change(&status);
                return DigestTick {
                    outcome: DigestOutcome::Inactive(status),
                    next_wake: poll,
                };
            }
            Err(error) => {
                // 送信済みかどうかを確認できないまま送ると二重送信になり得るので、送らない
                warn!(error = %error, "Failed to read notification state; daily digest skipped");
                return DigestTick {
                    outcome: DigestOutcome::Failed(error.to_string()),
                    next_wake: poll,
                };
            }
        };
        self.log_status_change(&NotificationStatus::active());

        let due_at = next_run(now, delivery.daily_digest_time, last_sent);
        if due_at > now {
            return DigestTick {
                outcome: DigestOutcome::NotDue,
                next_wake: due_at,
            };
        }
        if let Some(retry_at) = self.retry_not_before.filter(|retry_at| now < *retry_at) {
            return DigestTick {
                outcome: DigestOutcome::NotDue,
                next_wake: retry_at,
            };
        }

        match self.send(now, &delivery).await {
            Ok(()) => {
                self.retry_not_before = None;
                self.record_sent(now.date()).await;
                info!(
                    recipients = delivery.recipients.len(),
                    "Daily digest notification sent"
                );
                DigestTick {
                    next_wake: next_run(now, delivery.daily_digest_time, Some(now.date())),
                    outcome: DigestOutcome::Sent {
                        recipients: delivery.recipients,
                    },
                }
            }
            Err(error) => {
                let retry_at = now + Duration::seconds(RETRY_INTERVAL_SECS);
                self.retry_not_before = Some(retry_at);
                warn!(
                    error = %error,
                    retry_in_secs = RETRY_INTERVAL_SECS,
                    "Failed to send daily digest notification"
                );
                DigestTick {
                    outcome: DigestOutcome::Failed(error),
                    next_wake: retry_at,
                }
            }
        }
    }

    /// 壁時計基準で待機と判定を繰り返す
    pub async fn run(mut self) {
        loop {
            let now = chrono::Local::now().naive_local();
            let tick = self.tick(now).await;
            tokio::time::sleep(wait_duration(now, tick.next_wake)).await;
        }
    }

    /// 配送可否と、当日分を送信済みなら当日の日付（未送信なら永続化された最終送信日）を読む
    async fn read_state(
        &self,
        today: NaiveDate,
    ) -> RouterResult<(Result<Delivery, NotificationStatus>, Option<NaiveDate>)> {
        let snapshot = NotificationSnapshot::load(&self.pool, &self.credentials).await?;
        let persisted = self
            .settings
            .get_setting(KEY_DAILY_DIGEST_LAST_SENT_DATE)
            .await?
            .and_then(|value| NaiveDate::parse_from_str(&value, DATE_FORMAT).ok());
        let last_sent = if self.sent_on == Some(today) {
            Some(today)
        } else {
            persisted
        };
        Ok((snapshot.delivery, last_sent))
    }

    async fn send(&self, now: NaiveDateTime, delivery: &Delivery) -> Result<(), String> {
        let report = DigestReport::collect(&self.pool, now)
            .await
            .map_err(|error: LbError| error.to_string())?;
        let mail = render_daily_digest(delivery.language, &report);
        self.transport
            .send(&MailMessage {
                to: delivery.recipients.clone(),
                subject: mail.subject,
                body: mail.body,
            })
            .await
            .map_err(|error| error.to_string())
    }

    async fn record_sent(&mut self, date: NaiveDate) {
        self.sent_on = Some(date);
        let value = date.format(DATE_FORMAT).to_string();
        if let Err(error) = self
            .settings
            .set_setting(KEY_DAILY_DIGEST_LAST_SENT_DATE, &value)
            .await
        {
            warn!(
                error = %error,
                "Failed to persist the daily digest send date; a restart today may send it again"
            );
        }
    }

    fn log_status_change(&mut self, status: &NotificationStatus) {
        if self.last_status.as_ref() == Some(status) {
            return;
        }
        let reason = status.reason.as_deref().unwrap_or_default();
        match status.state {
            NotificationState::Active => info!("Operational notifications are active"),
            NotificationState::Disabled => {
                info!(reason = %reason, "Operational notifications are disabled")
            }
            NotificationState::Unavailable => warn!(
                reason = %reason,
                "Operational notifications are unavailable; no mail will be sent"
            ),
        }
        self.last_status = Some(status.clone());
    }
}

/// 次の判定までの待機時間（1 秒以上、[`POLL_INTERVAL_SECS`] 以下）
fn wait_duration(now: NaiveDateTime, next_wake: NaiveDateTime) -> std::time::Duration {
    let seconds = (next_wake - now).num_seconds().clamp(1, POLL_INTERVAL_SECS);
    std::time::Duration::from_secs(seconds.unsigned_abs())
}

/// 日次ダイジェストのバックグラウンドタスクを開始する
///
/// SMTP の認証情報は環境変数から読む。設定が未設定・不正でもタスクは起動し、
/// 送信だけを行わない。
pub fn start_daily_digest_task(pool: SqlitePool) {
    let credentials = SmtpCredentials::from_env();
    let transport = Arc::new(SmtpMailTransport::new(pool.clone(), credentials.clone()));
    tokio::spawn(DailyDigestScheduler::new(pool, transport, credentials).run());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn wait_is_capped_so_setting_changes_are_picked_up() {
        let now = at("2026-10-01 09:00:05");
        assert_eq!(
            wait_duration(now, at("2026-10-02 09:00:00")),
            std::time::Duration::from_secs(60)
        );
        assert_eq!(
            wait_duration(now, at("2026-10-01 09:00:35")),
            std::time::Duration::from_secs(30)
        );
    }

    #[test]
    fn wait_never_spins_when_the_wake_time_is_now_or_in_the_past() {
        let now = at("2026-10-01 09:00:05");
        assert_eq!(wait_duration(now, now), std::time::Duration::from_secs(1));
        assert_eq!(
            wait_duration(now, at("2026-10-01 08:00:00")),
            std::time::Duration::from_secs(1)
        );
    }
}
