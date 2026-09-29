//! OpenAI / Anthropic 互換の推論ルート（ルート直下）

use super::OPENAI_BODY_LIMIT_BYTES;
use crate::api::{anthropic, audio, images, openai, responses};
use crate::common::auth::ApiKeyPermission;
use crate::AppState;
use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
    Router,
};

/// OpenAI / Anthropic 互換ルート
pub(super) fn routes(state: &AppState) -> Router<AppState> {
    // APIキー認証が必要なルート（OpenAI互換の推論エンドポイント）
    let inference_routes = Router::new()
        .route("/v1/chat/completions", post(openai::chat_completions))
        .route("/v1/completions", post(openai::completions))
        .route("/v1/embeddings", post(openai::embeddings))
        // Open Responses API（SPEC-0f1de549）
        .route("/v1/responses", post(responses::post_responses))
        // 音声API（OpenAI Audio API互換）
        .route("/v1/audio/transcriptions", post(audio::transcriptions))
        .route("/v1/audio/speech", post(audio::speech))
        // 画像API（OpenAI Images API互換）
        .route("/v1/images/generations", post(images::generations))
        .route("/v1/images/edits", post(images::edits))
        .route("/v1/images/variations", post(images::variations))
        .layer(DefaultBodyLimit::max(OPENAI_BODY_LIMIT_BYTES));
    let inference_routes = inference_routes
        .layer(middleware::from_fn_with_state(
            ApiKeyPermission::OpenaiInference,
            crate::auth::middleware::require_api_key_permission_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.db_pool.clone(),
            crate::auth::middleware::api_key_auth_middleware,
        ));
    // Self-update drain gate: reject new inference requests and track in-flight requests.
    let inference_routes = inference_routes.layer(middleware::from_fn_with_state(
        state.lifecycle.inference_gate.clone(),
        crate::inference_gate::inference_gate_middleware,
    ));

    let anthropic_inference_routes = Router::new()
        .route("/v1/messages", post(anthropic::messages))
        .layer(DefaultBodyLimit::max(OPENAI_BODY_LIMIT_BYTES))
        .layer(middleware::from_fn_with_state(
            ApiKeyPermission::OpenaiInference,
            crate::auth::middleware::require_anthropic_api_key_permission_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.db_pool.clone(),
            crate::auth::middleware::anthropic_api_key_auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.lifecycle.inference_gate.clone(),
            crate::inference_gate::inference_gate_middleware,
        ));

    // `/v1/models*` は外部クライアント(APIキー)からのみ参照される
    // SPEC-e8e9326e: ノードトークン認証は廃止されました
    let models_routes = Router::new()
        .route("/v1/models", get(openai::list_models))
        .route("/v1/models/{model_id}", get(openai::get_model));
    let models_protected_routes = models_routes
        .layer(middleware::from_fn_with_state(
            ApiKeyPermission::OpenaiModelsRead,
            crate::auth::middleware::require_api_key_permission_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.db_pool.clone(),
            crate::auth::middleware::api_key_auth_middleware,
        ));

    Router::new()
        .merge(inference_routes)
        .merge(anthropic_inference_routes)
        .merge(models_protected_routes)
}
