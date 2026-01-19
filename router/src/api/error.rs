//! APIエラーレスポンス型
//!
//! axum用の共通エラーハンドリング

use axum::{http::StatusCode, response::IntoResponse, Json};
use llm_router_common::error::RouterError;
use serde_json::json;

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(pub RouterError);

impl From<RouterError> for AppError {
    fn from(err: RouterError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        // Use external_message() to avoid exposing internal details (IP addresses, ports, etc.)
        // Full error details are logged separately for debugging
        let (status, message) = match &self.0 {
            RouterError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.0.external_message()),
            RouterError::NoNodesAvailable => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            RouterError::NoCapableNodes(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            RouterError::ServiceUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            RouterError::NodeOffline(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.external_message())
            }
            RouterError::InvalidModelName(_) => {
                (StatusCode::BAD_REQUEST, self.0.external_message())
            }
            RouterError::InsufficientStorage(_) => {
                (StatusCode::INSUFFICIENT_STORAGE, self.0.external_message())
            }
            RouterError::Database(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message())
            }
            RouterError::Http(_) => (StatusCode::BAD_GATEWAY, self.0.external_message()),
            RouterError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.0.external_message()),
            RouterError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message())
            }
            RouterError::PasswordHash(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message())
            }
            RouterError::Jwt(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.external_message()),
            RouterError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.0.external_message()),
            RouterError::Authorization(_) => (StatusCode::FORBIDDEN, self.0.external_message()),
            RouterError::Common(err) => {
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
