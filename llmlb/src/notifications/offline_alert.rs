//! 即時通知（エンドポイントの Offline 到達）
//!
//! ダッシュボードイベントバスの `EndpointStatusChanged` を購読し、エンドポイントが
//! Offline に到達したときだけメールを送る。重複抑止の状態はプロセス内メモリで持つ:
//! 障害検知が目的なので、再起動直後に 1 通重複するほうが取りこぼしより安全である。

use super::config::{NotificationSnapshot, NotificationState, NotificationStatus, SmtpCredentials};
use super::digest::EndpointDigestEntry;
use super::mailer::{MailMessage, MailTransport};
use super::smtp::SmtpMailTransport;
use super::template::render_offline_alert;
use crate::events::{DashboardEvent, DashboardEventBus};
use crate::types::endpoint::EndpointStatus;
use chrono::NaiveDateTime;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast::{self, error::RecvError};
use tracing::{debug, info, warn};
use uuid::Uuid;

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
    pool: SqlitePool,
    transport: Arc<dyn MailTransport>,
    credentials: SmtpCredentials,
    /// 通知済みで、まだ Offline 以外の状態を経由していないエンドポイント
    alerted: HashSet<Uuid>,
}

impl OfflineAlertNotifier {
    /// 通知器を作成する
    pub fn new(
        pool: SqlitePool,
        transport: Arc<dyn MailTransport>,
        credentials: SmtpCredentials,
    ) -> Self {
        Self {
            pool,
            transport,
            credentials,
            alerted: HashSet::new(),
        }
    }

    /// イベントを 1 件処理し、Offline 到達なら `now`（サーバーのローカル時刻）を検知時刻として送信する
    pub async fn handle(&mut self, event: &DashboardEvent, now: NaiveDateTime) -> AlertOutcome {
        match *event {
            DashboardEvent::EndpointStatusChanged {
                runtime_id,
                old_status,
                new_status,
            } => {
                if new_status != EndpointStatus::Offline {
                    self.alerted.remove(&runtime_id);
                    return AlertOutcome::Ignored;
                }
                if old_status == EndpointStatus::Offline {
                    return AlertOutcome::Ignored;
                }
                if self.alerted.contains(&runtime_id) {
                    return AlertOutcome::Duplicate;
                }
                let outcome = self.alert(runtime_id, old_status, now).await;
                // 届いた通知だけを通知済みにする。送れなかった場合は次の Offline 到達でもう一度試みる
                if matches!(outcome, AlertOutcome::Sent { .. }) {
                    self.alerted.insert(runtime_id);
                }
                outcome
            }
            DashboardEvent::NodeRemoved { runtime_id } => {
                self.alerted.remove(&runtime_id);
                AlertOutcome::Ignored
            }
            _ => AlertOutcome::Ignored,
        }
    }

    /// イベントバスが閉じるまで、受信したイベントを順に処理する
    pub async fn run(mut self, mut events: broadcast::Receiver<DashboardEvent>) {
        loop {
            match events.recv().await {
                Ok(event) => {
                    self.handle(&event, chrono::Local::now().naive_local())
                        .await;
                }
                Err(RecvError::Lagged(skipped)) => {
                    // 復帰のイベントを見逃している可能性があるので、抑止の状態を捨てる
                    // （重複 1 通のほうが、障害の取りこぼしより安全）
                    warn!(
                        skipped,
                        "Offline alert task missed dashboard events; duplicate suppression was reset"
                    );
                    self.alerted.clear();
                }
                Err(RecvError::Closed) => return,
            }
        }
    }

    async fn alert(
        &self,
        endpoint_id: Uuid,
        previous_status: EndpointStatus,
        now: NaiveDateTime,
    ) -> AlertOutcome {
        let delivery = match NotificationSnapshot::load(&self.pool, &self.credentials).await {
            Ok(snapshot) => match snapshot.delivery {
                Ok(delivery) => delivery,
                Err(status) => {
                    log_not_sent(endpoint_id, &status);
                    return AlertOutcome::Inactive(status);
                }
            },
            Err(error) => return failed(endpoint_id, error.to_string()),
        };
        let endpoint = match crate::db::endpoints::get_endpoint(&self.pool, endpoint_id).await {
            Ok(Some(endpoint)) => endpoint,
            Ok(None) => {
                debug!(%endpoint_id, "Offline alert skipped; the endpoint is no longer registered");
                return AlertOutcome::Ignored;
            }
            Err(error) => return failed(endpoint_id, format!("Failed to read endpoint: {error}")),
        };

        let mail = render_offline_alert(
            delivery.language,
            &OfflineAlert {
                detected_at: now,
                previous_status,
                endpoint: EndpointDigestEntry::from(endpoint),
            },
        );
        let message = MailMessage {
            to: delivery.recipients,
            subject: mail.subject,
            body: mail.body,
        };
        match self.transport.send(&message).await {
            Ok(()) => {
                info!(
                    %endpoint_id,
                    recipients = message.to.len(),
                    "Offline alert notification sent"
                );
                AlertOutcome::Sent {
                    recipients: message.to,
                }
            }
            Err(error) => failed(endpoint_id, error.to_string()),
        }
    }
}

/// 通知が無効・未設定で送らなかったことをログに出す（意図して無効にしている場合は警告にしない）
fn log_not_sent(endpoint_id: Uuid, status: &NotificationStatus) {
    let reason = status.reason.as_deref().unwrap_or_default();
    if status.state == NotificationState::Unavailable {
        warn!(%endpoint_id, reason = %reason, "Offline alert was not sent; notifications are unavailable");
    } else {
        debug!(%endpoint_id, reason = %reason, "Offline alert was not sent; notifications are disabled");
    }
}

fn failed(endpoint_id: Uuid, error: String) -> AlertOutcome {
    warn!(%endpoint_id, error = %error, "Failed to send offline alert notification");
    AlertOutcome::Failed(error)
}

/// 即時通知のバックグラウンドタスクを開始する
///
/// SMTP の認証情報は環境変数から読む。設定が未設定・不正でもタスクは起動し、
/// 送信だけを行わない。購読は呼び出し時点で始まり、タスクはイベントバスが閉じるまで動き続ける。
pub fn start_offline_alert_task(pool: SqlitePool, event_bus: &DashboardEventBus) {
    let credentials = SmtpCredentials::from_env();
    let transport = Arc::new(SmtpMailTransport::new(pool.clone(), credentials.clone()));
    let events = event_bus.subscribe();
    tokio::spawn(OfflineAlertNotifier::new(pool, transport, credentials).run(events));
}
