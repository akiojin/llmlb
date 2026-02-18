//! Open Responses API エンドポイント (/v1/responses)
//!
//! SPEC-0f1de549: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! このモジュールは /v1/responses エンドポイントへのリクエストを
//! Responses API対応バックエンド（Ollama、vLLM、xLLM等）にパススルーする。

use crate::common::error::LbError;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderName, HeaderValue, StatusCode},
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
        proxy::{
            forward_streaming_response, forward_to_endpoint, record_endpoint_request_stats,
            select_available_endpoint_with_queue_for_model, QueueSelection,
        },
    },
    balancer::RequestOutcome,
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

fn queue_error_response(
    status: StatusCode,
    message: &str,
    error_type: &str,
    retry_after: Option<u64>,
) -> Response {
    let mut response = (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": status.as_u16(),
            }
        })),
    )
        .into_response();

    if let Some(value) = retry_after {
        if let Ok(header_value) = HeaderValue::from_str(&value.to_string()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static("retry-after"), header_value);
        }
    }

    response
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

    // モデルが未登録の場合は404、登録済みなら503（利用可能エンドポイントなし）
    if state
        .endpoint_registry
        .find_by_model(&model)
        .await
        .is_empty()
    {
        let is_registered = load_registered_model(&state.db_pool, &model).await?;
        if is_registered.is_none() {
            let message = format!("The model '{}' does not exist", model);
            return Ok(openai_error_response(message, StatusCode::NOT_FOUND));
        }
    }

    let queue_config = state.queue_config;

    // モデル対応エンドポイントをキュー付きで選択（モデル集合内で分散）
    let (endpoint, queued_wait_ms) =
        match select_available_endpoint_with_queue_for_model(&state, queue_config, &model).await {
            Ok(QueueSelection::Ready {
                endpoint,
                queued_wait_ms,
            }) => (*endpoint, queued_wait_ms),
            Ok(QueueSelection::CapacityExceeded) => {
                let retry_after = queue_config.timeout.as_secs().max(1);
                return Ok(queue_error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Request queue is full",
                    "rate_limit_exceeded",
                    Some(retry_after),
                ));
            }
            Ok(QueueSelection::Timeout { .. }) => {
                return Ok(queue_error_response(
                    StatusCode::GATEWAY_TIMEOUT,
                    "Queue wait timeout",
                    "timeout",
                    None,
                ));
            }
            Err(e) => {
                if matches!(e, LbError::NoCapableNodes(_)) {
                    let message = format!("No available endpoints support model: {}", model);
                    return Ok(model_unavailable_response(message));
                }
                return Err(AppError::from(e));
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

    let request_lease = state
        .load_manager
        .begin_request(endpoint.id)
        .await
        .map_err(AppError::from)?;

    // SPEC-f8e3a1b7: レイテンシ計測開始
    let start = Instant::now();

    // エンドポイントにリクエストを転送
    //
    // NOTE: Responses APIはレスポンス本文（ステータス含む）をそのまま返したい。
    // forward_to_endpoint() は stream=false の場合に非2xxをErr化するため、
    // ここでは常に "stream=true 相当"（= エラーもレスポンスとして受け取る）で呼び出す。
    let response =
        match forward_to_endpoint(&state.http_client, &endpoint, "/v1/responses", body, true).await
        {
            Ok(response) => response,
            Err(e) => {
                let duration = start.elapsed();
                request_lease
                    .complete(RequestOutcome::Error, duration)
                    .await
                    .map_err(AppError::from)?;
                record_endpoint_request_stats(
                    state.db_pool.clone(),
                    endpoint.id,
                    model.clone(),
                    false,
                );
                return Err(AppError::from(e));
            }
        };

    let duration = start.elapsed();
    let response_status = response.status();

    // ストリーミングの場合はそのままパススルー
    if stream {
        let outcome = if response_status.is_success() {
            RequestOutcome::Success
        } else {
            RequestOutcome::Error
        };
        let succeeded = response_status.is_success();
        request_lease
            .complete(outcome, duration)
            .await
            .map_err(AppError::from)?;

        record_endpoint_request_stats(state.db_pool.clone(), endpoint.id, model.clone(), succeeded);

        // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
        if response_status.is_success() {
            update_inference_latency(&state.endpoint_registry, endpoint.id, duration);
        }

        let mut axum_response = forward_streaming_response(response).map_err(AppError::from)?;
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut axum_response, wait_ms);
        }
        return Ok(axum_response);
    }

    // 非ストリーミングの場合
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            request_lease
                .complete(RequestOutcome::Error, duration)
                .await
                .map_err(AppError::from)?;
            record_endpoint_request_stats(state.db_pool.clone(), endpoint.id, model.clone(), false);
            return Err(AppError::from(LbError::Http(e.to_string())));
        }
    };

    let outcome = if status.is_success() {
        RequestOutcome::Success
    } else {
        RequestOutcome::Error
    };
    let succeeded = status.is_success();
    request_lease
        .complete(outcome, duration)
        .await
        .map_err(AppError::from)?;
    record_endpoint_request_stats(state.db_pool.clone(), endpoint.id, model.clone(), succeeded);

    // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
    if status.is_success() {
        update_inference_latency(&state.endpoint_registry, endpoint.id, duration);
    }

    // バックエンドのレスポンス（ステータス/ヘッダ/本文）をパススルー
    let mut axum_response = Response::new(Body::from(body_bytes));
    *axum_response.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    {
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

    if let Some(wait_ms) = queued_wait_ms {
        add_queue_headers(&mut axum_response, wait_ms);
    }

    Ok(axum_response)
}
