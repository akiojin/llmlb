//! APIエラーレスポンス型
//!
//! axum用の共通エラーハンドリング

use crate::common::error::{CommonError, LbError};
use axum::{response::IntoResponse, Json};
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
        let status = self.0.status_code();

        // Determine the user-facing message.
        // For errors that may contain internal details (IP addresses, ports, DB info),
        // use the generic external_message(). For user-facing errors where the message
        // is developer-crafted and safe to expose, use the actual error message.
        let message: String = match &self.0 {
            // May contain internal details (IPs, ports, DB info): use generic message
            LbError::Database(_)
            | LbError::Http(_)
            | LbError::Timeout(_)
            | LbError::Internal(_)
            | LbError::PasswordHash(_)
            | LbError::Jwt(_)
            | LbError::ServiceUnavailable(_)
            | LbError::EndpointNotFound(_)
            | LbError::NoEndpointsAvailable
            | LbError::NoCapableEndpoints(_)
            | LbError::EndpointOffline(_) => self.0.external_message().to_string(),

            // User-facing errors with developer-crafted messages safe to expose
            LbError::Common(CommonError::Validation(msg)) => msg.clone(),
            LbError::Common(err) => {
                // For other common errors (Config, Serialization, etc.), use generic
                let internal_message = err.to_string();
                if internal_message.contains("GPU is required")
                    || internal_message.contains("GPU hardware is required")
                {
                    internal_message
                } else {
                    self.0.external_message().to_string()
                }
            }
            LbError::Conflict(msg) => msg.clone(),
            LbError::NotFound(msg) => msg.clone(),
            LbError::Authorization(msg) => msg.clone(),
            LbError::Authentication(msg) => msg.clone(),
            LbError::InvalidModelName(msg) => msg.clone(),
            LbError::InsufficientStorage(msg) => msg.clone(),
        };

        let payload = json!({
            "error": message
        });

        (status, Json(payload)).into_response()
    }
}
