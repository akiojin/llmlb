//! Open Responses API エンドポイント (/v1/responses)
//!
//! SPEC-24157000: OpenAI互換API完全準拠 - Open Responses API対応
//!
//! このモジュールは /v1/responses エンドポイントへのリクエストを
//! Responses API対応バックエンド（Ollama、vLLM、xLLM等）にパススルーする。

use crate::common::error::LbError;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use tracing::{error, info};

use crate::{
    api::{
        error::AppError,
        proxy::{forward_streaming_response, forward_to_endpoint, EndpointSelection},
    },
    AppState,
};

/// 501 Not Implemented エラーレスポンス（バックエンドがResponses API非対応の場合）
fn not_implemented_response(model: &str) -> Response {
    let payload = json!({
        "error": {
            "message": format!(
                "Not Implemented: The backend for model '{}' does not support the Responses API",
                model
            ),
            "type": "server_error",
            "code": 501,
        }
    });

    (StatusCode::NOT_IMPLEMENTED, Json(payload)).into_response()
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

/// モデルIDからResponses API対応エンドポイントを選択
///
/// Responses API対応フラグが有効なエンドポイントのみを対象に、
/// レイテンシ順で最適なエンドポイントを選択する。
async fn select_endpoint_for_responses_api(
    state: &AppState,
    model_id: &str,
) -> Result<EndpointSelection, LbError> {
    let registry = &state.endpoint_registry;

    // モデルをサポートするオンラインエンドポイントを取得（レイテンシ順）
    let endpoints = registry.find_by_model_sorted_by_latency(model_id).await;

    // Responses API対応エンドポイントのみをフィルタリング
    for endpoint in endpoints {
        if endpoint.supports_responses_api {
            return Ok(EndpointSelection::Found(Box::new(endpoint)));
        }
    }

    Ok(EndpointSelection::NotFound)
}

/// POST /v1/responses - Open Responses API
///
/// リクエストをResponses API対応バックエンドにパススルーする。
/// バックエンドがResponses API非対応の場合は501 Not Implementedを返す。
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

    // Responses API対応エンドポイントを選択
    let endpoint = match select_endpoint_for_responses_api(&state, &model).await? {
        EndpointSelection::Found(ep) => ep,
        EndpointSelection::NotFound => {
            // モデルをサポートするエンドポイントが見つからない、または
            // Responses API非対応の場合は501を返す
            return Ok(not_implemented_response(&model));
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

    // エンドポイントにリクエストを転送
    let response =
        forward_to_endpoint(&state.http_client, &endpoint, "/v1/responses", body, stream)
            .await
            .map_err(AppError::from)?;

    // ストリーミングの場合はそのままパススルー
    if stream {
        return forward_streaming_response(response).map_err(AppError::from);
    }

    // 非ストリーミングの場合
    let status = response.status();
    let response_body = response.text().await.map_err(|e| {
        error!("Failed to read response body: {}", e);
        AppError::from(LbError::Http(e.to_string()))
    })?;

    // バックエンドが501を返した場合はそのままパススルー
    if status == reqwest::StatusCode::NOT_IMPLEMENTED {
        return Ok(not_implemented_response(&model));
    }

    // レスポンスをパース（エラーの場合もそのままパススルー）
    let response_json: Value = serde_json::from_str(&response_body).unwrap_or_else(|_| {
        json!({
            "error": {
                "message": "Invalid response from backend",
                "type": "server_error",
                "raw": response_body
            }
        })
    });

    let axum_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    Ok((axum_status, Json(response_json)).into_response())
}
