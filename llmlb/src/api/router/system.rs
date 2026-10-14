//! システム（バージョン・自己更新）のルート

use crate::api::system;
use crate::AppState;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};

/// `/api` 配下のシステムルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // システムAPI（更新状態/適用）
    // GET /api/system は認証不要（FR-006: バージョン情報を常時表示するため）
    // POST /api/system/update/* は JWT + CSRF で保護
    let system_mutation_routes = Router::new()
        .route("/system/update/check", post(system::check_update))
        .route("/system/update/apply", post(system::apply_update))
        .route(
            "/system/update/apply/force",
            post(system::apply_force_update),
        )
        .route(
            "/system/update/schedule",
            post(system::create_schedule)
                .get(system::get_schedule)
                .delete(system::cancel_schedule),
        )
        .route("/system/update/rollback", post(system::rollback));
    let system_mutation_routes = system_mutation_routes
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::middleware::require_jwt_auth_middleware,
        ));

    Router::new()
        // 認証不要エンドポイント
        .route("/version", get(system::get_version))
        // GET /api/system: 認証不要（FR-006: バージョン・更新状態を常時取得可能にする）
        .route("/system", get(system::get_system))
        .merge(system_mutation_routes)
}
