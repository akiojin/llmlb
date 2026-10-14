//! 認証・アカウント管理（ログイン/ユーザー/APIキー/招待）のルート

use crate::api::{api_keys, auth, invitations, users};
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
        .route("/users", get(users::list_users).post(users::create_user))
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

    let invitations_routes = Router::new()
        .route(
            "/invitations",
            get(invitations::list_invitations).post(invitations::create_invitation),
        )
        .route("/invitations/{id}", delete(invitations::revoke_invitation))
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::InvitationsManage,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    Router::new()
        // 認証エンドポイント（ログインは認証不要）
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .merge(auth_routes)
        .merge(users_routes)
        .merge(my_api_keys_routes)
        .merge(invitations_routes)
}
