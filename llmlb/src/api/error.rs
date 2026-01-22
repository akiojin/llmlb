//! APIエラーレスポンス型
//!
//! axum用の共通エラーハンドリング

use axum::{http::StatusCode, response::IntoResponse, Json};
use llmlb_common::error::LbError;
use serde_json::json;

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(pub LbError);

impl From<LbError> for AppError {
    fn from(err: LbError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        // Use external_message() to avoid exposing internal details (IP addresses, ports, etc.)
        // Full error details are logged separately for debugging
        let (status, message) = match &self.0 {
            LbError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.0.external_message()),
            LbError::NoNodesAvailable => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            LbError::NoCapableNodes(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            LbError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            LbError::NodeOffline(_) => (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message()),
            LbError::InvalidModelName(_) => (StatusCode::BAD_REQUEST, self.0.external_message()),
            LbError::InsufficientStorage(_) => {
                (StatusCode::INSUFFICIENT_STORAGE, self.0.external_message())
            }
            LbError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message()),
            LbError::Http(_) => (StatusCode::BAD_GATEWAY, self.0.external_message()),
            LbError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.0.external_message()),
            LbError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message()),
            LbError::PasswordHash(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message())
            }
            LbError::Jwt(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message()),
            LbError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.0.external_message()),
            LbError::Authorization(_) => (StatusCode::FORBIDDEN, self.0.external_message()),
            LbError::Common(err) => {
                // GPU必須エラーの場合は403 Forbiddenを返す
                let internal_message = err.to_string();
                if internal_message.contains("GPU is required")
                    || internal_message.contains("GPU hardware is required")
                {
                    (StatusCode::FORBIDDEN, self.0.external_message())
                } else {
                    (StatusCode::BAD_REQUEST, self.0.external_message())
                }
            }
        };

        let payload = json!({
            "error": message
        });

        (status, Json(payload)).into_response()
    }
}
