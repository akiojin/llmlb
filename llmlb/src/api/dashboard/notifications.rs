//! 運用通知の設定 API（SPEC #777）
//!
//! ダッシュボードの通知設定 UI 向け。管理者が有効／無効・SMTP の非秘密設定・送信時刻・言語を
//! 確認・変更し、宛先（email 設定済みの管理者）と、送信できない場合の理由を確認できる。
//! SMTP の認証情報は環境変数でのみ設定でき、この API では扱わない。

use crate::api::error::AppError;
use crate::common::error::{CommonError, LbError};
use crate::db::settings::SettingsStorage;
use crate::db::users::NotificationRecipient;
use crate::notifications::config::KEY_DAILY_DIGEST_LAST_SENT_DATE;
use crate::notifications::{
    save_settings, NotificationSettings, NotificationSnapshot, NotificationStatus, SmtpCredentials,
};
use crate::AppState;
use axum::{extract::State, Json};
use serde::Serialize;

/// GET / PUT /api/dashboard/notifications のレスポンス
#[derive(Debug, Serialize)]
pub struct NotificationSettingsResponse {
    /// 通知設定（非秘密）
    pub settings: NotificationSettings,
    /// 通知機能の状態と、送信できない場合の理由
    pub status: NotificationStatus,
    /// SMTP の認証情報が環境変数に設定されているか（値そのものは返さない）
    pub credentials_configured: bool,
    /// 宛先（email 設定済みの管理者）
    pub recipients: Vec<NotificationRecipient>,
    /// 日次ダイジェストの最終送信日（`YYYY-MM-DD`、未送信なら `null`）
    pub last_digest_sent_date: Option<String>,
}

async fn current_settings(state: &AppState) -> Result<NotificationSettingsResponse, AppError> {
    let credentials = SmtpCredentials::from_env();
    let snapshot = NotificationSnapshot::load(&state.db_pool, &credentials)
        .await
        .map_err(AppError)?;
    let last_digest_sent_date = SettingsStorage::new(state.db_pool.clone())
        .get_setting(KEY_DAILY_DIGEST_LAST_SENT_DATE)
        .await
        .map_err(AppError)?;

    Ok(NotificationSettingsResponse {
        status: snapshot.status(),
        settings: snapshot.loaded.settings,
        credentials_configured: credentials.is_complete(),
        recipients: snapshot.recipients,
        last_digest_sent_date,
    })
}

/// GET /api/dashboard/notifications - 通知設定と状態の取得（admin のみ）
pub async fn get_notification_settings(
    State(state): State<AppState>,
) -> Result<Json<NotificationSettingsResponse>, AppError> {
    current_settings(&state).await.map(Json)
}

/// PUT /api/dashboard/notifications - 通知設定の更新（admin のみ）
///
/// 形式が不正な値は 400 で拒否する。SMTP が未設定のまま有効化することは拒否せず、
/// 送信できない理由をレスポンスの `status` で返す。
pub async fn update_notification_settings(
    State(state): State<AppState>,
    Json(settings): Json<NotificationSettings>,
) -> Result<Json<NotificationSettingsResponse>, AppError> {
    let settings = settings
        .validated()
        .map_err(|message| AppError(LbError::Common(CommonError::Validation(message))))?;
    save_settings(&SettingsStorage::new(state.db_pool.clone()), &settings)
        .await
        .map_err(AppError)?;

    current_settings(&state).await.map(Json)
}
