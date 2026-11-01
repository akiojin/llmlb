//! Admin-only management API for the shared text inference default.

use crate::api::error::AppError;
use crate::common::error::{CommonError, LbError};
use crate::inference_defaults::storage::{self, InferenceDefaults};
use crate::AppState;
use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use serde_json::Value;

/// GET /api/dashboard/inference-defaults.
pub async fn get_inference_defaults(
    State(state): State<AppState>,
) -> Result<Json<InferenceDefaults>, AppError> {
    storage::load(&state.db_pool)
        .await
        .map(Json)
        .map_err(AppError)
}

/// PUT /api/dashboard/inference-defaults, committing before reporting success.
pub async fn update_inference_defaults(
    State(state): State<AppState>,
    body: Result<Json<Value>, JsonRejection>,
) -> Result<Json<InferenceDefaults>, AppError> {
    let Json(value) = body.map_err(|_| {
        AppError(LbError::Common(CommonError::Validation(
            "Expected a JSON object containing default_text_model".to_string(),
        )))
    })?;
    let defaults = InferenceDefaults::from_value(value).map_err(AppError)?;
    storage::save(&state, &defaults).await.map_err(AppError)?;
    Ok(Json(defaults))
}
