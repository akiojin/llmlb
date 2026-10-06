//! OpenAI互換APIエンドポイント (/v1/*)
//!
//! このモジュールはEndpointRegistry/Endpoint型を使用しています。

/// 未指定/仮想IPアドレス（クラウドプロバイダ等、実IPを持たない場合に使用）
const UNSPECIFIED_IP: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);

mod catalog;
mod cloud;
mod proxy_post;
#[cfg(test)]
mod tests;

pub use catalog::{get_model, list_models};
#[cfg(test)]
use cloud::cloud_virtual_node;
use cloud::{parse_cloud_model, proxy_openai_cloud_post};
use proxy_post::proxy_openai_post;

use crate::common::{
    error::{CommonError, LbError},
    protocol::RequestType,
};
use crate::types::model::ModelCapability;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderName, HeaderValue},
    response::Response,
    Json,
};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use uuid::Uuid;

use crate::auth::middleware::ApiKeyAuthContext;
use crate::common::ip::{normalize_ip, normalize_socket_ip};

use crate::{
    api::{
        error::{AppError, OpenAIError},
        model_name::{parse_quantized_model_name, ParsedModelName},
        models::list_registered_models,
    },
    AppState,
};

/// SPEC-f8e3a1b7: 推論リクエスト成功時にエンドポイントのレイテンシを更新（Fire-and-forget）
fn update_inference_latency(
    registry: &crate::registry::endpoints::EndpointRegistry,
    endpoint_id: Uuid,
    duration: std::time::Duration,
) {
    let registry = registry.clone();
    let latency_ms = duration.as_millis() as f64;
    tokio::spawn(async move {
        if let Err(e) = registry
            .update_inference_latency(endpoint_id, latency_ms)
            .await
        {
            tracing::debug!(
                endpoint_id = %endpoint_id,
                latency_ms = latency_ms,
                error = %e,
                "Failed to update inference latency"
            );
        }
    });
}

fn add_queue_headers(response: &mut Response, wait_ms: u128) {
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-queue-status"),
        HeaderValue::from_static("queued"),
    );
    let wait_value = wait_ms.to_string();
    if let Ok(value) = HeaderValue::from_str(&wait_value) {
        headers.insert(HeaderName::from_static("x-queue-wait-ms"), value);
    }
}

/// クライアントIPとAPIキーIDを抽出するヘルパー
fn extract_client_info(
    addr: &SocketAddr,
    headers: &HeaderMap,
    auth_ctx: &Option<axum::Extension<ApiKeyAuthContext>>,
) -> (Option<IpAddr>, Option<Uuid>) {
    let client_ip =
        Some(extract_client_ip_from_headers(headers).unwrap_or_else(|| normalize_socket_ip(addr)));
    let api_key_id = auth_ctx.as_ref().map(|ext| ext.0.id);
    (client_ip, api_key_id)
}

fn extract_client_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    extract_x_forwarded_for(headers).or_else(|| extract_forwarded_for(headers))
}

fn extract_x_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let value = headers.get("x-forwarded-for")?.to_str().ok()?;
    value
        .split(',')
        .map(str::trim)
        .find_map(parse_client_ip_from_forwarded_value)
}

fn extract_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    value.split(',').find_map(|entry| {
        entry
            .split(';')
            .filter_map(|pair| pair.split_once('='))
            .find_map(|(key, value)| {
                if key.trim().eq_ignore_ascii_case("for") {
                    parse_client_ip_from_forwarded_value(value.trim())
                } else {
                    None
                }
            })
    })
}

fn parse_client_ip_from_forwarded_value(value: &str) -> Option<IpAddr> {
    let trimmed = value.trim().trim_matches('"');
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") || trimmed.starts_with('_') {
        return None;
    }

    let host = if let Some(stripped) = trimmed.strip_prefix('[') {
        stripped.split(']').next().unwrap_or_default().trim()
    } else {
        trimmed
    };

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(normalize_ip(ip));
    }

    if let Some((ip_candidate, _port)) = host.rsplit_once(':') {
        if !ip_candidate.contains(':') {
            if let Ok(ip) = ip_candidate.parse::<IpAddr>() {
                return Some(normalize_ip(ip));
            }
        }
    }

    None
}

/// POST /v1/chat/completions - OpenAI互換チャットAPI
pub async fn chat_completions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, OpenAIError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model(&payload)?;
    let parsed = if parse_cloud_model(&model).is_some() {
        ParsedModelName {
            raw: model.clone(),
            base: model.clone(),
            quantization: None,
        }
    } else {
        parse_quantized_model_name(&model).map_err(OpenAIError::from)?
    };
    let requires_image_input = payload_requires_image_input(&payload);

    // モデルの TextGeneration capability を検証
    let models = list_registered_models(&state.db_pool).await?;
    if let Some(model_info) = models.iter().find(|m| m.name == model) {
        if !model_info.has_capability(ModelCapability::TextGeneration) {
            return Err(OpenAIError::from(LbError::Common(CommonError::Validation(
                format!("Model '{}' does not support text generation", parsed.raw),
            ))));
        }
        if requires_image_input && !model_info.has_capability(ModelCapability::ImageInput) {
            return Err(OpenAIError::from(LbError::Common(CommonError::Validation(
                format!("Model '{}' does not support image input", parsed.raw),
            ))));
        }
    }
    // 登録されていないモデルはエンドポイント側で処理（クラウドモデル等）

    let stream = extract_stream(&payload);
    proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        parsed.raw,
        stream,
        RequestType::Chat,
        client_ip,
        api_key_id,
    )
    .await
}

/// POST /api/dashboard/playground/chat/completions - Dashboardセッション用チャットAPI
///
/// LB Playgroundからの試行リクエストを、APIキーではなくJWTセッションで受け付ける。
/// 外部クライアント向けの`/v1/chat/completions`は引き続きAPIキー必須。
pub async fn dashboard_playground_chat_completions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
    chat_completions(
        ConnectInfo(addr),
        headers,
        State(state),
        None,
        Json(payload),
    )
    .await
    .map_err(AppError::from)
}

/// GET /api/dashboard/playground/models keeps the management error exit.
pub async fn dashboard_playground_models(
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    list_models(State(state)).await.map_err(AppError::from)
}

/// POST /v1/completions - OpenAI互換テキスト補完API
pub async fn completions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, OpenAIError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model(&payload)?;
    if parse_cloud_model(&model).is_none() {
        parse_quantized_model_name(&model).map_err(OpenAIError::from)?;
    }
    let stream = extract_stream(&payload);
    proxy_openai_post(
        &state,
        payload,
        "/v1/completions",
        model,
        stream,
        RequestType::Generate,
        client_ip,
        api_key_id,
    )
    .await
}

/// POST /v1/embeddings - OpenAI互換Embeddings API
pub async fn embeddings(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, OpenAIError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model_with_default(&payload, crate::config::get_default_embedding_model());
    if parse_cloud_model(&model).is_none() {
        parse_quantized_model_name(&model).map_err(OpenAIError::from)?;
    }
    proxy_openai_post(
        &state,
        payload,
        "/v1/embeddings",
        model,
        false,
        RequestType::Embeddings,
        client_ip,
        api_key_id,
    )
    .await
}

fn extract_model(payload: &Value) -> Result<String, OpenAIError> {
    payload
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| validation_error("`model` field is required for OpenAI-compatible requests"))
}

/// モデル名を抽出し、未指定または空の場合はデフォルト値を使用
fn extract_model_with_default(payload: &Value, default: String) -> String {
    payload
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or(default)
}

fn extract_stream(payload: &Value) -> bool {
    payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn payload_requires_image_input(payload: &Value) -> bool {
    let Some(messages) = payload.get("messages").and_then(|v| v.as_array()) else {
        return false;
    };

    for message in messages {
        let Some(parts) = message.get("content").and_then(|v| v.as_array()) else {
            continue;
        };
        for part in parts {
            let part_type = part.get("type").and_then(|v| v.as_str());
            if matches!(part_type, Some("image_url" | "input_image")) {
                return true;
            }
        }
    }

    false
}

fn validation_error(message: impl Into<String>) -> OpenAIError {
    let err = LbError::Common(CommonError::Validation(message.into()));
    err.into()
}
