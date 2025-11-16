//! エージェント側のOpenAI互換エンドポイント
//! 受け取ったリクエストをローカルのOllamaにプロキシする

use crate::api::models::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use tracing::error;

fn proxy_error(e: impl std::fmt::Display) -> StatusCode {
    error!("Failed to proxy to local Ollama: {}", e);
    StatusCode::BAD_GATEWAY
}

/// 共通プロキシ処理
async fn proxy_to_ollama(
    state: &AppState,
    path: &str,
    body: Option<Value>,
) -> Result<Response, StatusCode> {
    let client = reqwest::Client::new();
    let ollama_base = {
        // lock短時間
        let mgr = state.ollama_manager.lock().await;
        mgr.api_base()
    };
    let url = format!("{}/{}", ollama_base.trim_end_matches('/'), path.trim_start_matches('/'));
    let mut req = client.post(url);
    if let Some(json) = body {
        req = req.json(&json);
    }
    let resp = req.send().await.map_err(proxy_error)?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let bytes = resp.bytes().await.map_err(proxy_error)?;
    Ok((status, bytes).into_response())
}

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    proxy_to_ollama(&state, "/api/chat", Some(body)).await
}

/// POST /v1/completions
pub async fn completions(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    proxy_to_ollama(&state, "/api/generate", Some(body)).await
}

/// POST /v1/embeddings
pub async fn embeddings(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    proxy_to_ollama(&state, "/api/embed", Some(body)).await
}

/// GET /v1/models
pub async fn list_models(State(state): State<AppState>) -> Result<Response, StatusCode> {
    // Ollamaの /api/tags はオブジェクト形式だが、ここでは OpenAI互換の最小レスポンスに整形する
    let client = reqwest::Client::new();
    let ollama_base = {
        let mgr = state.ollama_manager.lock().await;
        mgr.api_base()
    };
    let url = format!("{}/api/tags", ollama_base.trim_end_matches('/'));
    let resp = client.get(url).send().await.map_err(proxy_error)?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let json: serde_json::Value = resp.json().await.map_err(proxy_error)?;
    // 期待フォーマット: { "models": [ { "name": "..."} ] }
    let models = json
        .get("models")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let data: Vec<Value> = models
        .into_iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(|id| {
            serde_json::json!({
                "id": id,
                "object": "model",
                "owned_by": "agent",
            })
        }))
        .collect();
    let body = serde_json::json!({
        "object": "list",
        "data": data,
    });
    Ok((status, Json(body)).into_response())
}
