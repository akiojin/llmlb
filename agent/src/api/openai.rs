//! エージェント側のOpenAI互換エンドポイント
//! 受け取ったリクエストをローカルのOllamaにプロキシする

use crate::{api::models::AppState, ollama_pool::OllamaPool};
use axum::{
    body::Body,
    extract::State,
    http::{StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures::TryStreamExt;
use serde_json::Value;
use tracing::error;

fn proxy_error(e: impl std::fmt::Display) -> StatusCode {
    error!("Failed to proxy to local Ollama: {}", e);
    StatusCode::BAD_GATEWAY
}

/// 共通プロキシ処理
async fn proxy_to_ollama(
    state: &AppState,
    model: Option<String>,
    path: &str,
    body: Option<Value>,
) -> Result<Response, StatusCode> {
    let client = reqwest::Client::new();

    // モデルに応じて Ollama ポートを確保
    let target_url = if let Some(m) = model {
        let pool: &OllamaPool = &state.ollama_pool;
        let port = pool.ensure(&m).await.map_err(proxy_error)?;
        format!("http://127.0.0.1:{}/{}", port, path.trim_start_matches('/'))
    } else {
        // モデル不明の場合はデフォルトOLLAMA（初期ポート）へ
        let mgr = state.ollama_manager.lock().await;
        let ollama_base = mgr.api_base();
        format!("{}/{}", ollama_base.trim_end_matches('/'), path.trim_start_matches('/'))
    };
    let mut req = client.post(target_url);
    if let Some(json) = body {
        req = req.json(&json);
    }
    let resp = req.send().await.map_err(proxy_error)?;
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    // ストリーム対応: Content-Type が text/event-stream または chunked の場合はボディごと転送
    if resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/event-stream"))
        .unwrap_or(false)
    {
        let stream = resp.bytes_stream().map_err(|e| {
            error!("Stream error: {}", e);
            std::io::Error::new(std::io::ErrorKind::Other, e)
        });
        let body = Body::from_stream(stream);
        let mut response = Response::new(body);
        *response.status_mut() = status;
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/event-stream"),
        );
        return Ok(response);
    }

    let bytes = resp.bytes().await.map_err(proxy_error)?;
    Ok((status, bytes).into_response())
}

/// POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    let model = body.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    proxy_to_ollama(&state, model, "/api/chat", Some(body)).await
}

/// POST /v1/completions
pub async fn completions(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    let model = body.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    proxy_to_ollama(&state, model, "/api/generate", Some(body)).await
}

/// POST /v1/embeddings
pub async fn embeddings(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Response, StatusCode> {
    let model = body.get("model").and_then(|v| v.as_str()).map(|s| s.to_string());
    proxy_to_ollama(&state, model, "/api/embed", Some(body)).await
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
        "initializing": state.initializing().await,
        "ready_models": state.ready_models().await,
    });
    Ok((status, Json(body)).into_response())
}
