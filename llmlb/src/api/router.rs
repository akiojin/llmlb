//! ルーティング（FR-012: create_app() をドメイン別 Router で構成する）

mod auth;
mod dashboard;
mod endpoints;
mod inference;
mod metrics;
mod models;
mod system;
#[cfg(test)]
mod tests;

use crate::AppState;
use axum::{http::StatusCode, middleware, Router};

/// OpenAI互換推論リクエストのボディ上限
const OPENAI_BODY_LIMIT_BYTES: usize = 20 * 1024 * 1024;

/// APIllmlbを作成
pub fn create_app(state: AppState) -> Router {
    // `/api/*`: llmlb独自API（管理/運用向け）
    let api_routes = Router::new()
        .merge(system::routes(&state))
        .merge(auth::routes(&state))
        .merge(dashboard::api_routes(&state))
        .merge(endpoints::routes(&state))
        .merge(models::routes(&state))
        .merge(metrics::routes(&state));

    // SPEC-e8e9326e: POST /api/health（プッシュ型ヘルスチェック）は廃止されました
    // 新しいエンドポイントはプル型ヘルスチェック（EndpointHealthChecker）を使用

    Router::new()
        // `/api/*`: llmlb独自API（互換不要・versioned）
        .nest("/api", api_routes)
        // OpenAI互換API
        .merge(inference::routes(&state))
        .merge(dashboard::ui_routes())
        // NOTE: Playground機能は廃止され、ダッシュボード内のエンドポイント別Playgroundに移行
        // /playground/* ルートは削除済み
        .fallback(|| async { StatusCode::NOT_FOUND })
        // 監査ログミドルウェア (SPEC-8301d106): 全リクエストをキャプチャ（最外層）
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::audit::middleware::audit_middleware,
        ))
        .with_state(state)
}
