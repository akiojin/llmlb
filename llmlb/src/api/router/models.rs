//! モデル管理・モデル配布レジストリのルート

use crate::api::models;
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::AppState;
use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};

/// `/api` 配下のモデルルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // モデル管理API (Admin のみ: register/delete)
    let models_manage_routes = Router::new()
        .route("/models/register", post(models::register_model))
        .route("/models/{*model_name}", delete(models::delete_model))
        .layer(middleware::from_fn(
            crate::auth::middleware::require_password_changed_middleware,
        ))
        .layer(middleware::from_fn(
            crate::auth::middleware::csrf_protect_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            crate::auth::middleware::JwtOrApiKeyPermissionConfig {
                app_state: state.clone(),
                required_permission: ApiKeyPermission::ModelsManage,
                jwt_required_role: Some(UserRole::Admin),
                api_key_role: UserRole::Admin,
            },
            crate::auth::middleware::jwt_or_api_key_permission_middleware,
        ));

    // モデル配布レジストリ（registry.read が必要）
    // SPEC-e8e9326e: POST /api/nodes（ノード自己登録）は廃止されました
    // 新しい実装は POST /api/endpoints を使用してください
    let model_registry_routes = Router::new()
        // モデル配布レジストリ（複数ファイル: safetensors 等）
        .route(
            "/models/registry/{model_name}/manifest.json",
            get(models::get_model_registry_manifest),
        );
    let model_registry_routes = model_registry_routes
        .layer(middleware::from_fn_with_state(
            ApiKeyPermission::RegistryRead,
            crate::auth::middleware::require_api_key_permission_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.db_pool.clone(),
            crate::auth::middleware::api_key_auth_middleware,
        ));

    // モデル一覧API (Admin OR Runtime スコープで利用可能)
    // /api/models はランタイム同期用の登録済みモデル一覧
    // /api/models/hub はダッシュボード向けの対応モデル一覧 + ステータス
    let models_list_routes = {
        let cfg = crate::auth::middleware::JwtOrApiKeyPermissionConfig {
            app_state: state.clone(),
            required_permission: ApiKeyPermission::RegistryRead,
            jwt_required_role: Some(UserRole::Admin),
            api_key_role: UserRole::Viewer,
        };
        Router::new()
            .route("/models", get(models::list_models))
            .route("/models/hub", get(models::list_models_with_status))
            .layer(middleware::from_fn_with_state(
                cfg,
                crate::auth::middleware::jwt_or_api_key_permission_middleware,
            ))
    };

    // NOTE: /api/models (GET) は Admin/Node スコープ共用。
    // 外部クライアントは /v1/models を使用してください（Azure OpenAI 形式の capabilities 付き）。

    Router::new()
        .merge(models_manage_routes)
        .merge(model_registry_routes)
        .merge(models_list_routes)
}
