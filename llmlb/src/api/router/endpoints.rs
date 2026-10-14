//! エンドポイント管理のルート

use crate::api::{dashboard, endpoints, logs};
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::AppState;
use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};

/// `/api` 配下のエンドポイント管理ルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // エンドポイントログ取得（lb→endpoint proxy）
    let node_logs_routes = Router::new()
        .route("/endpoints/{id}/logs", get(logs::get_endpoint_logs))
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::LogsRead,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    // エンドポイント管理API（SPEC-e8e9326e）
    // READ: endpoints.read
    // WRITE: endpoints.manage (JWTはadminのみ)
    let endpoint_read_routes = Router::new()
        .route("/endpoints", get(endpoints::list_endpoints))
        .route("/endpoints/{id}", get(endpoints::get_endpoint))
        .route(
            "/endpoints/{id}/models",
            get(endpoints::list_endpoint_models),
        )
        .route(
            "/endpoints/{id}/download/progress",
            get(endpoints::download_progress),
        )
        .route(
            "/endpoints/{id}/models/{model}/info",
            get(endpoints::get_model_info),
        )
        // SPEC-8c32349f: エンドポイント単位リクエスト統計
        .route(
            "/endpoints/{id}/today-stats",
            get(dashboard::get_endpoint_today_stats),
        )
        .route(
            "/endpoints/{id}/daily-stats",
            get(dashboard::get_endpoint_daily_stats),
        )
        .route(
            "/endpoints/{id}/model-stats",
            get(dashboard::get_endpoint_model_stats),
        )
        // SPEC-4bb5b55f: エンドポイント×モデル単位TPS
        .route(
            "/endpoints/{id}/model-tps",
            get(dashboard::get_endpoint_model_tps),
        );
    let endpoint_read_routes = endpoint_read_routes
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::EndpointsRead,
                jwt_required_role: None,
                api_key_role: UserRole::Viewer,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    let endpoint_manage_routes = Router::new()
        .route("/endpoints", post(endpoints::create_endpoint))
        .route(
            "/endpoints/{id}",
            put(endpoints::update_endpoint).delete(endpoints::delete_endpoint),
        )
        .route("/endpoints/{id}/test", post(endpoints::test_endpoint))
        .route(
            "/endpoints/{id}/sync",
            post(endpoints::sync_endpoint_models),
        )
        // SPEC-e8e9326e: ダウンロードAPI
        .route("/endpoints/{id}/download", post(endpoints::download_model))
        // モデル削除API（POST: モデル名にスラッシュが含まれるため）
        .route(
            "/endpoints/{id}/models/delete",
            post(endpoints::delete_endpoint_model_handler),
        );
    let endpoint_manage_routes = endpoint_manage_routes
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::EndpointsManage,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));
    // Playground用プロキシ（JWT認証のみ、APIキー不可）
    // ダッシュボードにログインしているユーザーのみがエンドポイントに直接リクエストを転送できる
    let playground_proxy_routes = Router::new().route(
        "/endpoints/{id}/chat/completions",
        post(endpoints::proxy_chat_completions),
    );
    let playground_proxy_routes = playground_proxy_routes
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
    // Treat dashboard playground proxy as inference for drain purposes.
    let playground_proxy_routes = playground_proxy_routes.layer(middleware::from_fn_with_state(
        state.lifecycle.inference_gate.clone(),
        crate::inference_gate::inference_gate_middleware,
    ));

    Router::new()
        .merge(node_logs_routes)
        .merge(endpoint_read_routes)
        .merge(endpoint_manage_routes)
        .merge(playground_proxy_routes)
}
