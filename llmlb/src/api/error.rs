//! APIエラーレスポンス型
//!
//! axum用の共通エラーハンドリング

use crate::common::error::{CommonError, LbError};
use axum::{
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(pub LbError);

/// OpenAI compatibility handlers use the existing safe, structured converter.
#[derive(Debug)]
pub struct OpenAIError(pub LbError);

impl From<LbError> for OpenAIError {
    fn from(error: LbError) -> Self {
        Self(error)
    }
}

impl From<AppError> for OpenAIError {
    fn from(error: AppError) -> Self {
        Self(error.0)
    }
}

impl From<OpenAIError> for AppError {
    fn from(error: OpenAIError) -> Self {
        Self(error.0)
    }
}

impl IntoResponse for OpenAIError {
    fn into_response(self) -> Response {
        (self.0.status_code(), Json(self.0.to_openai_error())).into_response()
    }
}

/// Management handlers opt in to an additive machine-readable error code.
/// Legacy management exits continue using `AppError`.
#[derive(Debug)]
pub struct ManagementError(pub LbError);

impl From<LbError> for ManagementError {
    fn from(error: LbError) -> Self {
        Self(error)
    }
}

impl From<AppError> for ManagementError {
    fn from(error: AppError) -> Self {
        Self(error.0)
    }
}

impl IntoResponse for ManagementError {
    fn into_response(self) -> Response {
        let status = self.0.status_code();
        let code = self.0.code();
        let message = AppError(self.0).message();
        (status, Json(json!({"error": message, "code": code}))).into_response()
    }
}

/// ハンドラ/ミドルウェアの `Err` 用に `Response` を Box 化した軽量エラー型
///
/// `Result<_, Response>` は `Err` が大きく `clippy::result_large_err` に抵触するため、
/// ポインタ1個分のサイズで同じレスポンスをそのまま返す。
#[derive(Debug)]
pub struct HandlerError(Box<Response>);

impl From<Response> for HandlerError {
    fn from(response: Response) -> Self {
        HandlerError(Box::new(response))
    }
}

impl From<AppError> for HandlerError {
    fn from(err: AppError) -> Self {
        err.into_response().into()
    }
}

impl IntoResponse for HandlerError {
    fn into_response(self) -> Response {
        *self.0
    }
}

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

    #[test]
    fn test_handler_error_is_pointer_sized() {
        // clippy::result_large_err の閾値 (128 bytes) を十分下回ること
        assert_eq!(
            std::mem::size_of::<HandlerError>(),
            std::mem::size_of::<usize>()
        );
    }

    #[tokio::test]
    async fn test_handler_error_preserves_response() {
        let original = AppError(LbError::NotFound("missing".into())).into_response();
        let resp = HandlerError::from(original).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "missing");
    }

    #[tokio::test]
    async fn test_handler_error_from_app_error_matches_app_error_response() {
        let resp =
            HandlerError::from(AppError(LbError::Authorization("denied".into()))).into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
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
        let message = self.message();

        let payload = json!({
            "error": message
        });

        (status, Json(payload)).into_response()
    }
}

#[allow(clippy::items_after_test_module)]
impl AppError {
    fn message(&self) -> String {
        // Determine the user-facing message.
        // For errors that may contain internal details (IP addresses, ports, DB info),
        // use the generic external_message(). For user-facing errors where the message
        // is developer-crafted and safe to expose, use the actual error message.
        match &self.0 {
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
            LbError::DuplicateUrl(msg) => msg.clone(),
            LbError::NotFound(msg) => msg.clone(),
            LbError::Authorization(msg) => msg.clone(),
            LbError::Authentication(msg) => msg.clone(),
            LbError::InvalidModelName(msg) => msg.clone(),
            LbError::InsufficientStorage(msg) => msg.clone(),
        }
    }
}
