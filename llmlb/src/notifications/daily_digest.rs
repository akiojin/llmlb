//! 日次ダイジェストのスケジューラ
//!
//! 既存の定期処理（`tokio::time::interval`）はプロセス起動基準で、再起動すると実行時刻が
//! ずれる。日次ダイジェストは壁時計基準で次回実行時刻を求めて待機し、最終送信日を
//! `settings` に永続化することで、再起動をまたいでも時刻がずれず同日に二重送信しない。

use super::config::{NotificationStatus, SmtpCredentials};
use super::mailer::MailTransport;
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::SqlitePool;
use std::sync::Arc;

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
        let _ = (
            now,
            &self.pool,
            &self.transport,
            &self.credentials,
            &self.last_status,
            &self.retry_not_before,
            &self.sent_on,
        );
        todo!("SPEC #777 T-004 / T-005")
    }

    /// 壁時計基準で待機と判定を繰り返す
    pub async fn run(mut self) {
        let _ = self.tick(chrono::Local::now().naive_local()).await;
        todo!("SPEC #777 T-004")
    }
}

/// 日次ダイジェストのバックグラウンドタスクを開始する
///
/// SMTP の認証情報は環境変数から読む。設定が未設定・不正でもタスクは起動し、
/// 送信だけを行わない。
pub fn start_daily_digest_task(pool: SqlitePool) {
    let _ = pool;
    todo!("SPEC #777 T-004")
}
