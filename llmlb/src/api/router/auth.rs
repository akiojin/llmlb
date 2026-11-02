//! 認証・アカウント管理（ログイン/ユーザー/APIキー/登録申請）のルート

use crate::api::{api_keys, auth, organizations, registration_requests, users};
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::AppState;
use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};

/// `/api` 配下の認証・アカウント管理ルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // JWTが必要な認証ルート（ログイン以外）
    let auth_routes = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/change-password", put(auth::change_password))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::middleware::require_jwt_auth_middleware,
        ));

    // 管理系API（運用/自動化向け）
    //
    // 許可される認証:
    // - JWT (admin role)
    // - APIキー (必要な permissions を保有)
    //
    // NOTE: /api/dashboard/* は管理UI向けのため JWT のみ（APIキー不可）
    let users_routes = Router::new()
        .route("/users", post(users::create_user))
        .route("/registration-requests", get(registration_requests::list))
        .route(
            "/registration-requests/{id}/approve",
            post(registration_requests::approve),
        )
        .route(
            "/registration-requests/{id}",
            delete(registration_requests::reject),
        )
        .route(
            "/users/{id}",
            put(users::update_user).delete(users::delete_user),
        )
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::UsersManage,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    // 一覧はJWTのDB scopeを適用する。legacy UsersManage自動化は既存DTOのまま維持する。
    let users_read_routes = Router::new()
        .route("/users", get(users::list_users))
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::UsersManage,
                jwt_required_role: None,
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    let organization_read_routes = Router::new()
        .route("/organizations", get(organizations::list_organizations))
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::middleware::require_jwt_auth_middleware,
        ));

    // 新しい組織境界の変更にはAPIキーや組織内adminを使わせない。
    let organization_manage_routes = Router::new()
        .route("/organizations", post(organizations::create_organization))
        .route(
            "/organizations/{id}",
            put(organizations::rename_organization).delete(organizations::archive_organization),
        )
        .route("/users/{id}/membership", put(users::update_membership))
        .route("/users/{id}/disabled", put(users::update_disabled))
        .layer(middleware::from_fn(organizations::require_global_scope))
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

    // ユーザー自身のAPIキー管理（JWTのみ）
    let my_api_keys_routes = Router::new()
        .route(
            "/me/api-keys",
            get(api_keys::list_api_keys).post(api_keys::create_api_key),
        )
        .route(
            "/me/api-keys/{id}",
            put(api_keys::update_api_key).delete(api_keys::delete_api_key),
        )
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
        // 認証エンドポイント（ログインは認証不要）
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .merge(auth_routes)
        .merge(users_routes)
        .merge(users_read_routes)
        .merge(organization_read_routes)
        .merge(organization_manage_routes)
        .merge(my_api_keys_routes)
}
