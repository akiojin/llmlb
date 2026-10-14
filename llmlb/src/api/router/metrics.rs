//! メトリクス export のルート

use crate::cloud_metrics;
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::AppState;
use axum::{middleware, routing::get, Router};

/// `/api` 配下のメトリクスルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // Prometheus metrics（cloud prefix含む独自メトリクス）
    Router::new()
        .route("/metrics/cloud", get(cloud_metrics::export_metrics))
        // SPEC #582 US-005: provider/日次 rollup の JSON/CSV export
        .route(
            "/metrics/cloud/export",
            get(cloud_metrics::export_daily_metrics),
        )
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::MetricsRead,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ))
}
