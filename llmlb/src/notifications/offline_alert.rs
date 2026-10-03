//! 即時通知（エンドポイントの Offline 到達）
//!
//! ダッシュボードイベントバスの `EndpointStatusChanged` を購読し、エンドポイントが
//! Offline に到達したときだけメールを送る。重複抑止の状態はプロセス内メモリで持つ:
//! 障害検知が目的なので、再起動直後に 1 通重複するほうが取りこぼしより安全である。

use super::config::{NotificationStatus, SmtpCredentials};
use super::digest::EndpointDigestEntry;
use super::mailer::MailTransport;
use crate::events::{DashboardEvent, DashboardEventBus};
use crate::types::endpoint::EndpointStatus;
use chrono::NaiveDateTime;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 即時通知 1 通分の内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineAlert {
    /// 検知時刻（サーバーのローカル時刻）
    pub detected_at: NaiveDateTime,
    /// Offline に到達する直前の状態
    pub previous_status: EndpointStatus,
    /// Offline に到達したエンドポイント
    pub endpoint: EndpointDigestEntry,
}

/// イベント 1 件の処理結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertOutcome {
    /// 送信した
    Sent {
        /// 宛先アドレス
        recipients: Vec<String>,
    },
    /// 通知の対象ではない（Offline 到達以外のイベント、または登録が消えたエンドポイント）
    Ignored,
    /// 通知済みで、まだ Offline 以外の状態を経由していない
    Duplicate,
    /// 通知が無効、または設定の未設定・不正で送信できない
    Inactive(NotificationStatus),
    /// 送信に失敗した
    Failed(String),
}

/// Offline 到達の即時通知
pub struct OfflineAlertNotifier {
    _pool: SqlitePool,
    _transport: Arc<dyn MailTransport>,
    _credentials: SmtpCredentials,
}

impl OfflineAlertNotifier {
    /// 通知器を作成する
    pub fn new(
        pool: SqlitePool,
        transport: Arc<dyn MailTransport>,
        credentials: SmtpCredentials,
    ) -> Self {
        Self {
            _pool: pool,
            _transport: transport,
            _credentials: credentials,
        }
    }

    /// イベントを 1 件処理し、Offline 到達なら `now`（サーバーのローカル時刻）を検知時刻として送信する
    pub async fn handle(&mut self, _event: &DashboardEvent, _now: NaiveDateTime) -> AlertOutcome {
        AlertOutcome::Ignored
    }

    /// イベントバスが閉じるまで、受信したイベントを順に処理する
    pub async fn run(self, _events: broadcast::Receiver<DashboardEvent>) {}
}

/// 即時通知のバックグラウンドタスクを開始する
pub fn start_offline_alert_task(_pool: SqlitePool, _event_bus: &DashboardEventBus) {}
