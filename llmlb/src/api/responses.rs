//! Open Responses API エンドポイント (/v1/responses)
//!
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! このモジュールは /v1/responses エンドポイントへのリクエストを
//! Responses API対応バックエンド（Ollama、vLLM、xLLM等）にパススルーする。

use crate::common::error::LbError;
use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::time::Instant;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    api::{
        error::AppError,
        models::load_registered_model,
        proxy::{forward_streaming_response, forward_to_endpoint},
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

fn openai_error_response(message: impl Into<String>, status: StatusCode) -> Response {
    let payload = json!({
        "error": {
            "message": message.into(),
            "type": "invalid_request_error",
            "code": status.as_u16(),
        }
    });

    (status, Json(payload)).into_response()
}

fn model_unavailable_response(message: impl Into<String>) -> Response {
    let payload = json!({
        "error": {
            "message": message.into(),
            "type": "service_unavailable",
            "code": "no_capable_nodes",
        }
    });

    (StatusCode::SERVICE_UNAVAILABLE, Json(payload)).into_response()
}

/// リクエストからモデル名を抽出
fn extract_model(payload: &Value) -> Result<String, AppError> {
    payload["model"].as_str().map(String::from).ok_or_else(|| {
        AppError::from(LbError::Common(
            crate::common::error::CommonError::Validation("Missing required field: model".into()),
        ))
    })
}

/// リクエストからstreamフラグを抽出
fn extract_stream(payload: &Value) -> bool {
    payload["stream"].as_bool().unwrap_or(false)
}

/// POST /v1/responses - Open Responses API
///
/// リクエストをバックエンドにパススルーする（判定/フラグは廃止）。
pub async fn post_responses(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
    let model = extract_model(&payload)?;
    let stream = extract_stream(&payload);

    info!(
        model = %model,
        stream = stream,
        "Processing Responses API request"
    );

    // モデルをサポートするオンラインエンドポイントを取得（レイテンシ順）
    let endpoint = match state
        .endpoint_registry
        .find_by_model_sorted_by_latency(&model)
        .await
        .into_iter()
        .next()
    {
        Some(ep) => ep,
        None => {
            // モデルが未登録の場合は404、登録済みなら503（利用可能エンドポイントなし）
            let is_registered = load_registered_model(&state.db_pool, &model).await?;
            if is_registered.is_none() {
                let message = format!("The model '{}' does not exist", model);
                return Ok(openai_error_response(message, StatusCode::NOT_FOUND));
            }
            let message = format!("No available endpoints support model: {}", model);
            return Ok(model_unavailable_response(message));
        }
    };

    info!(
        endpoint_id = %endpoint.id,
        endpoint_name = %endpoint.name,
        "Forwarding to Responses API endpoint"
    );

    // リクエストボディをそのままパススルー
    let body = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize request: {}", e);
        AppError::from(LbError::Http(e.to_string()))
    })?;

    // SPEC-f8e3a1b7: レイテンシ計測開始
    let start = Instant::now();

    // エンドポイントにリクエストを転送
    //
    // NOTE: Responses APIはレスポンス本文（ステータス含む）をそのまま返したい。
    // forward_to_endpoint() は stream=false の場合に非2xxをErr化するため、
    // ここでは常に "stream=true 相当"（= エラーもレスポンスとして受け取る）で呼び出す。
    let response = forward_to_endpoint(&state.http_client, &endpoint, "/v1/responses", body, true)
        .await
        .map_err(AppError::from)?;

    let duration = start.elapsed();

    // ストリーミングの場合はそのままパススルー
    if stream {
        // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
        update_inference_latency(&state.endpoint_registry, endpoint.id, duration);
        return forward_streaming_response(response).map_err(AppError::from);
    }

    // 非ストリーミングの場合
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = response.bytes().await.map_err(|e| {
        error!("Failed to read response body: {}", e);
        AppError::from(LbError::Http(e.to_string()))
    })?;

    // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
    if status.is_success() {
        update_inference_latency(&state.endpoint_registry, endpoint.id, duration);
    }

    // バックエンドのレスポンス（ステータス/ヘッダ/本文）をパススルー
    let mut axum_response = Response::new(Body::from(body_bytes));
    *axum_response.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    {
        use axum::http::{HeaderName, HeaderValue};

        let response_headers = axum_response.headers_mut();
        for (name, value) in headers.iter() {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(name.as_str().as_bytes()),
                HeaderValue::from_bytes(value.as_bytes()),
            ) {
                response_headers.insert(header_name, header_value);
            }
        }
    }
    Ok(axum_response)
}
