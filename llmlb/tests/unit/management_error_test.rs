//! Issue #827: 管理APIの固定codeと共有エラー応答の互換性。

use axum::{body::to_bytes, response::IntoResponse};
use llmlb::{
    api::error::{AppError, ManagementError},
    common::error::{CommonError, LbError},
};
use serde_json::json;
use uuid::Uuid;

fn cases() -> Vec<(LbError, u16, &'static str, &'static str)> {
    let detail = "private 10.0.0.1:8080 /secret/db.sqlite";
    vec![
        (
            LbError::Common(CommonError::Config(detail.into())),
            400,
            "Request error",
            "config_error",
        ),
        (
            LbError::Common(CommonError::Serialization(
                serde_json::from_str::<serde_json::Value>("{").unwrap_err(),
            )),
            400,
            "Request error",
            "serialization_error",
        ),
        (
            LbError::Common(CommonError::UuidParse(Uuid::parse_str("bad").unwrap_err())),
            400,
            "Request error",
            "uuid_parse_error",
        ),
        (
            LbError::Common(CommonError::IpAddrParse(
                "bad".parse::<std::net::IpAddr>().unwrap_err(),
            )),
            400,
            "Request error",
            "ip_addr_parse_error",
        ),
        (
            LbError::Common(CommonError::Validation("invalid input".into())),
            400,
            "invalid input",
            "validation_error",
        ),
        (
            LbError::Common(CommonError::Config("GPU is required".into())),
            400,
            "Configuration error: GPU is required",
            "config_error",
        ),
        (
            LbError::EndpointNotFound(Uuid::nil()),
            404,
            "Endpoint not found",
            "endpoint_not_found",
        ),
        (
            LbError::NotFound("missing".into()),
            404,
            "missing",
            "not_found",
        ),
        (
            LbError::NoEndpointsAvailable,
            503,
            "No available endpoints",
            "no_endpoints_available",
        ),
        (
            LbError::NoCapableEndpoints(detail.into()),
            404,
            "No capable endpoints",
            "no_capable_endpoints",
        ),
        (
            LbError::Database(detail.into()),
            500,
            "Database error",
            "database_error",
        ),
        (
            LbError::Http(detail.into()),
            502,
            "Backend service unavailable",
            "http_error",
        ),
        (
            LbError::Timeout(detail.into()),
            504,
            "Request timeout",
            "timeout",
        ),
        (
            LbError::ServiceUnavailable(detail.into()),
            503,
            "Service temporarily unavailable",
            "service_unavailable",
        ),
        (
            LbError::Internal(detail.into()),
            500,
            "Internal server error",
            "internal_error",
        ),
        (
            LbError::EndpointOffline(Uuid::nil()),
            503,
            "Endpoint offline",
            "endpoint_offline",
        ),
        (
            LbError::InvalidModelName("bad model".into()),
            400,
            "bad model",
            "invalid_model_name",
        ),
        (
            LbError::InsufficientStorage("storage full".into()),
            507,
            "storage full",
            "insufficient_storage",
        ),
        (
            LbError::PasswordHash(detail.into()),
            401,
            "Authentication error",
            "password_hash_error",
        ),
        (
            LbError::Jwt(detail.into()),
            401,
            "Authentication error",
            "jwt_error",
        ),
        (
            LbError::Authentication("bad credentials".into()),
            401,
            "bad credentials",
            "authentication_error",
        ),
        (
            LbError::Authorization("denied".into()),
            403,
            "denied",
            "authorization_error",
        ),
        (
            LbError::Conflict("duplicate".into()),
            409,
            "duplicate",
            "conflict",
        ),
        (
            LbError::DuplicateUrl("duplicate".into()),
            409,
            "duplicate",
            "duplicate_url",
        ),
    ]
}

#[tokio::test]
async fn all_management_codes_are_fixed_and_messages_match_legacy() {
    for (error, status, message, code) in cases() {
        assert_eq!(error.code(), code);
        assert!(code.bytes().all(|c| c.is_ascii_lowercase() || c == b'_'));
        let response = ManagementError::from(error).into_response();
        assert_eq!(response.status().as_u16(), status);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({"error": message, "code": code})
        );
    }
    for (error, status, message, _) in cases() {
        let response = AppError(error).into_response();
        assert_eq!(response.status().as_u16(), status);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            body.as_ref(),
            serde_json::to_vec(&json!({"error": message})).unwrap()
        );
    }
}

#[tokio::test]
async fn question_mark_converts_common_and_app_errors_to_management_errors() {
    fn from_common() -> Result<(), ManagementError> {
        Err::<(), _>(LbError::NotFound("missing".into()))?;
        Ok(())
    }
    fn from_app() -> Result<(), ManagementError> {
        Err::<(), _>(AppError(LbError::NotFound("missing".into())))?;
        Ok(())
    }
    for error in [from_common().unwrap_err(), from_app().unwrap_err()] {
        let body = to_bytes(error.into_response().into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            json!({"error": "missing", "code": "not_found"})
        );
    }
}
