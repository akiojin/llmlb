//! APIエラーレスポンス型
//!
//! axum用の共通エラーハンドリング

use crate::common::error::{CommonError, LbError};
use axum::{response::IntoResponse, Json};
use serde_json::json;

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(pub LbError);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use uuid::Uuid;

    /// ヘルパー: AppError -> (StatusCode, body JSON)
    async fn response_parts(err: LbError) -> (StatusCode, serde_json::Value) {
        let resp = AppError(err).into_response();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn test_database_error_returns_500() {
        let (status, body) = response_parts(LbError::Database("conn failed".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        // Should NOT leak internal message
        assert_eq!(body["error"], "Database error");
    }

    #[tokio::test]
    async fn test_http_error_returns_502() {
        let (status, body) = response_parts(LbError::Http("upstream down".into())).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body["error"], "Backend service unavailable");
    }

    #[tokio::test]
    async fn test_timeout_error_returns_504() {
        let (status, body) = response_parts(LbError::Timeout("30s exceeded".into())).await;
        assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(body["error"], "Request timeout");
    }

    #[tokio::test]
    async fn test_internal_error_returns_500() {
        let (status, body) = response_parts(LbError::Internal("panic".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"], "Internal server error");
    }

    #[tokio::test]
    async fn test_password_hash_error_returns_401() {
        let (status, body) = response_parts(LbError::PasswordHash("bcrypt fail".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "Authentication error");
    }

    #[tokio::test]
    async fn test_jwt_error_returns_401() {
        let (status, body) = response_parts(LbError::Jwt("expired".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "Authentication error");
    }

    #[tokio::test]
    async fn test_service_unavailable_returns_503() {
        let (status, body) =
            response_parts(LbError::ServiceUnavailable("initializing".into())).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "Service temporarily unavailable");
    }

    #[tokio::test]
    async fn test_endpoint_not_found_returns_404() {
        let id = Uuid::new_v4();
        let (status, body) = response_parts(LbError::EndpointNotFound(id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Endpoint not found");
    }

    #[tokio::test]
    async fn test_no_endpoints_available_returns_503() {
        let (status, body) = response_parts(LbError::NoEndpointsAvailable).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "No available endpoints");
    }

    #[tokio::test]
    async fn test_no_capable_endpoints_returns_404() {
        let (status, body) = response_parts(LbError::NoCapableEndpoints("llama3".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "No capable endpoints");
    }

    #[tokio::test]
    async fn test_endpoint_offline_returns_503() {
        let id = Uuid::new_v4();
        let (status, body) = response_parts(LbError::EndpointOffline(id)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "Endpoint offline");
    }

    #[tokio::test]
    async fn test_common_validation_returns_400_with_message() {
        let msg = "name is required".to_string();
        let (status, body) =
            response_parts(LbError::Common(CommonError::Validation(msg.clone()))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], msg);
    }

    #[tokio::test]
    async fn test_conflict_returns_409_with_message() {
        let msg = "endpoint already exists".to_string();
        let (status, body) = response_parts(LbError::Conflict(msg.clone())).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], msg);
    }

    #[tokio::test]
    async fn test_not_found_returns_404_with_message() {
        let msg = "model not found".to_string();
        let (status, body) = response_parts(LbError::NotFound(msg.clone())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], msg);
    }

    #[tokio::test]
    async fn test_authorization_returns_403_with_message() {
        let msg = "insufficient permissions".to_string();
        let (status, body) = response_parts(LbError::Authorization(msg.clone())).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], msg);
    }
}

#[allow(clippy::items_after_test_module)]
impl From<LbError> for AppError {
    fn from(err: LbError) -> Self {
        AppError(err)
    }
}

#[allow(clippy::items_after_test_module)]
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
