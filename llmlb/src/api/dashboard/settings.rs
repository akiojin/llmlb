//! ダッシュボード設定 API（閾値設定の取得・更新）

use crate::api::error::AppError;
use crate::common::error::{CommonError, LbError};
use crate::AppState;
use axum::{extract::State, Json};
use serde::Deserialize;
use tracing::warn;

/// 設定APIのデフォルト値
pub(super) const IP_ALERT_THRESHOLD_DEFAULT_VALUE: i64 = 100;
pub(super) const IP_ALERT_THRESHOLD_MIN: i64 = 1;

pub(super) fn parse_ip_alert_threshold(value: &str) -> Result<i64, LbError> {
    let parsed = value.trim().parse::<i64>().map_err(|_| {
        LbError::Common(CommonError::Validation(
            "ip_alert_threshold must be an integer >= 1".to_string(),
        ))
    })?;
    if parsed < IP_ALERT_THRESHOLD_MIN {
        return Err(LbError::Common(CommonError::Validation(
            "ip_alert_threshold must be an integer >= 1".to_string(),
        )));
    }
    Ok(parsed)
}

pub(super) fn effective_ip_alert_threshold(raw_value: Option<&str>) -> i64 {
    match raw_value {
        Some(raw) => match parse_ip_alert_threshold(raw) {
            Ok(value) => value,
            Err(err) => {
                warn!(
                    raw_value = %raw,
                    error = %err,
                    default = IP_ALERT_THRESHOLD_DEFAULT_VALUE,
                    "Invalid ip_alert_threshold in settings; falling back to default"
                );
                IP_ALERT_THRESHOLD_DEFAULT_VALUE
            }
        },
        None => IP_ALERT_THRESHOLD_DEFAULT_VALUE,
    }
}

/// 専用 API が管理するキーを、この汎用 API から読み書きさせない
///
/// `notifications.*`（SPEC #777）は admin 専用の `/api/dashboard/notifications` が検証付きで
/// 管理する。汎用 API は viewer でも読めて検証もしないため、対象外にする。
fn reject_reserved_key(key: &str) -> Result<(), LbError> {
    if key.starts_with(crate::notifications::config::KEY_PREFIX) {
        return Err(LbError::Authorization(
            "notifications.* settings are managed through /api/dashboard/notifications".to_string(),
        ));
    }
    Ok(())
}

/// GET /api/dashboard/settings/{key} - 設定値取得
///
/// SPEC-62ac4b68: 閾値ベースの異常検知
pub async fn get_setting(
    axum::extract::Path(key): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    reject_reserved_key(&key).map_err(AppError)?;
    let settings = crate::db::settings::SettingsStorage::new(state.db_pool.clone());
    let value = settings.get_setting(&key).await.map_err(AppError)?;
    let value = if key == "ip_alert_threshold" {
        effective_ip_alert_threshold(value.as_deref()).to_string()
    } else {
        value.unwrap_or_default()
    };
    Ok(Json(serde_json::json!({ "key": key, "value": value })))
}

/// PUT /api/dashboard/settings/{key} のリクエストボディ
#[derive(Debug, Deserialize)]
pub struct SettingUpdateBody {
    /// 設定値
    pub value: String,
}

/// PUT /api/dashboard/settings/{key} - 設定値更新
///
/// SPEC-62ac4b68: 閾値ベースの異常検知
pub async fn update_setting(
    axum::Extension(claims): axum::Extension<crate::common::auth::Claims>,
    axum::extract::Path(key): axum::extract::Path<String>,
    State(state): State<AppState>,
    Json(body): Json<SettingUpdateBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 設定の書き込みは admin のみ（Viewer の権限昇格を防ぐ）
    if claims.role != crate::common::auth::UserRole::Admin {
        return Err(AppError(LbError::Authorization(
            "Only admin can update settings".to_string(),
        )));
    }
    reject_reserved_key(&key).map_err(AppError)?;
    let value = if key == "ip_alert_threshold" {
        parse_ip_alert_threshold(&body.value)
            .map_err(AppError)?
            .to_string()
    } else {
        body.value
    };

    let settings = crate::db::settings::SettingsStorage::new(state.db_pool.clone());
    settings.set_setting(&key, &value).await.map_err(AppError)?;
    Ok(Json(serde_json::json!({ "key": key, "value": value })))
}
