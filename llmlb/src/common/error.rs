//! エラー型定義
//!
//! 統一エラー型（thiserror使用）
//!
//! # OpenAI互換エラーレスポンス
//!
//! `LbError`は`error_type()`と`status_code()`メソッドを提供し、
//! OpenAI互換のエラーレスポンスを生成できます。

use axum::http::StatusCode;
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// Common layer error type
#[derive(Debug, Error)]
pub enum CommonError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// UUID parse error
    #[error("UUID parse error: {0}")]
    UuidParse(#[from] uuid::Error),

    /// IP address parse error
    #[error("IP address parse error: {0}")]
    IpAddrParse(#[from] std::net::AddrParseError),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),
}

/// load balancer error type
#[derive(Debug, Error)]
pub enum LbError {
    /// Common layer error
    #[error(transparent)]
    Common(#[from] CommonError),

    /// Runtime not found
    #[error("Runtime not found: {0}")]
    NodeNotFound(Uuid),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// No available runtimes
    #[error("No available runtimes")]
    NoNodesAvailable,

    /// No capable runtimes for model
    #[error("No capable runtimes for model: {0}")]
    NoCapableNodes(String),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// HTTP client error
    #[error("HTTP client error: {0}")]
    Http(String),

    /// Timeout error
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Service unavailable (e.g., during initialization)
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Runtime is offline
    #[error("Runtime {0} is offline")]
    NodeOffline(Uuid),

    /// Invalid model name
    #[error("Invalid model name: {0}")]
    InvalidModelName(String),

    /// Insufficient storage
    #[error("Insufficient storage: {0}")]
    InsufficientStorage(String),

    /// Password hash error
    #[error("Password hash error: {0}")]
    PasswordHash(String),

    /// JWT error
    #[error("JWT error: {0}")]
    Jwt(String),

    /// Authentication error
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Authorization error
    #[error("Authorization error: {0}")]
    Authorization(String),
}

impl LbError {
    /// Returns a safe error message for external clients.
    ///
    /// This method returns a generic error message that does not expose
    /// internal implementation details such as IP addresses, port numbers,
    /// or internal service names. Use this for HTTP responses to external clients.
    ///
    /// For debugging purposes, use the `Display` implementation (`to_string()`)
    /// which includes full error details - but only in server logs.
    pub fn external_message(&self) -> &'static str {
        match self {
            Self::Common(_) => "Request error",
            Self::NodeNotFound(_) => "Runtime not found",
            Self::NotFound(_) => "Not found",
            Self::NoNodesAvailable => "No available runtimes",
            Self::NoCapableNodes(_) => "No capable runtimes",
            Self::Database(_) => "Database error",
            Self::Http(_) => "Backend service unavailable",
            Self::Timeout(_) => "Request timeout",
            Self::ServiceUnavailable(_) => "Service temporarily unavailable",
            Self::Internal(_) => "Internal server error",
            Self::NodeOffline(_) => "Runtime offline",
            Self::InvalidModelName(_) => "Invalid model name",
            Self::InsufficientStorage(_) => "Insufficient storage",
            Self::PasswordHash(_) => "Authentication error",
            Self::Jwt(_) => "Authentication error",
            Self::Authentication(_) => "Authentication failed",
            Self::Authorization(_) => "Access denied",
        }
    }

    /// Returns the OpenAI-compatible error type string.
    ///
    /// # Error Types
    ///
    /// - `invalid_request_error`: Bad request parameters
    /// - `authentication_error`: Auth failures
    /// - `permission_error`: Authorization failures
    /// - `not_found_error`: Resource not found
    /// - `rate_limit_error`: Too many requests
    /// - `server_error`: Internal server errors
    /// - `service_unavailable`: Backend unavailable
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::Common(CommonError::Validation(_)) => "invalid_request_error",
            Self::Common(_) => "invalid_request_error",
            Self::NodeNotFound(_) => "not_found_error",
            Self::NotFound(_) => "not_found_error",
            Self::NoNodesAvailable => "service_unavailable",
            Self::NoCapableNodes(_) => "not_found_error",
            Self::Database(_) => "server_error",
            Self::Http(_) => "service_unavailable",
            Self::Timeout(_) => "server_error",
            Self::ServiceUnavailable(_) => "service_unavailable",
            Self::Internal(_) => "server_error",
            Self::NodeOffline(_) => "service_unavailable",
            Self::InvalidModelName(_) => "invalid_request_error",
            Self::InsufficientStorage(_) => "server_error",
            Self::PasswordHash(_) => "authentication_error",
            Self::Jwt(_) => "authentication_error",
            Self::Authentication(_) => "authentication_error",
            Self::Authorization(_) => "permission_error",
        }
    }

    /// Returns the HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Common(CommonError::Validation(_)) => StatusCode::BAD_REQUEST,
            Self::Common(_) => StatusCode::BAD_REQUEST,
            Self::NodeNotFound(_) => StatusCode::NOT_FOUND,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::NoNodesAvailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::NoCapableNodes(_) => StatusCode::NOT_FOUND,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Http(_) => StatusCode::BAD_GATEWAY,
            Self::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NodeOffline(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::InvalidModelName(_) => StatusCode::BAD_REQUEST,
            Self::InsufficientStorage(_) => StatusCode::INSUFFICIENT_STORAGE,
            Self::PasswordHash(_) => StatusCode::UNAUTHORIZED,
            Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Authentication(_) => StatusCode::UNAUTHORIZED,
            Self::Authorization(_) => StatusCode::FORBIDDEN,
        }
    }

    /// Converts this error to an OpenAI-compatible error response.
    pub fn to_openai_error(&self) -> OpenAIErrorResponse {
        OpenAIErrorResponse {
            error: OpenAIErrorDetail {
                message: self.external_message().to_string(),
                error_type: self.error_type().to_string(),
                code: Some(self.status_code().as_u16().to_string()),
            },
        }
    }
}

/// OpenAI互換エラーレスポンス
///
/// # Example
///
/// ```json
/// {
///   "error": {
///     "message": "No available runtimes",
///     "type": "service_unavailable",
///     "code": "503"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct OpenAIErrorResponse {
    /// The error details
    pub error: OpenAIErrorDetail,
}

/// OpenAIエラー詳細
#[derive(Debug, Clone, Serialize)]
pub struct OpenAIErrorDetail {
    /// Human-readable error message
    pub message: String,
    /// Error type (e.g., "invalid_request_error", "server_error")
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error code (optional, typically HTTP status as string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Runtime error type
#[derive(Debug, Error)]
pub enum NodeError {
    /// Common layer error
    #[error(transparent)]
    Common(#[from] CommonError),

    /// load balancer connection error
    #[error("Failed to connect to Router: {0}")]
    RouterConnection(String),

    /// LLM runtime connection error
    #[error("Failed to connect to LLM runtime: {0}")]
    RuntimeConnection(String),

    /// Registration error
    #[error("Runtime registration failed: {0}")]
    Registration(String),

    /// Health check send error
    #[error("Failed to send health check: {0}")]
    Heartbeat(String),

    /// Metrics collection error
    #[error("Failed to collect metrics: {0}")]
    Metrics(String),

    /// GUI error
    #[error("GUI error: {0}")]
    Gui(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias (Common)
pub type CommonResult<T> = Result<T, CommonError>;

/// Result type alias (load balancer)
pub type RouterResult<T> = Result<T, LbError>;

/// Result type alias (Runtime)
pub type NodeResult<T> = Result<T, NodeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_error_display() {
        let error = CommonError::Config("test config error".to_string());
        assert_eq!(error.to_string(), "Configuration error: test config error");
    }

    #[test]
    fn test_lb_error_node_not_found() {
        let node_id = Uuid::new_v4();
        let error = LbError::NodeNotFound(node_id);
        assert!(error.to_string().contains(&node_id.to_string()));
    }

    #[test]
    fn test_lb_error_no_nodes() {
        let error = LbError::NoNodesAvailable;
        assert_eq!(error.to_string(), "No available runtimes");
    }

    #[test]
    fn test_node_error_router_connection() {
        let error = NodeError::RouterConnection("timeout".to_string());
        assert_eq!(error.to_string(), "Failed to connect to Router: timeout");
    }

    #[test]
    fn test_error_from_conversion() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let common_error: CommonError = json_error.into();
        assert!(matches!(common_error, CommonError::Serialization(_)));
    }

    #[test]
    fn test_lb_error_type() {
        // Authentication errors
        assert_eq!(
            LbError::Authentication("test".to_string()).error_type(),
            "authentication_error"
        );
        assert_eq!(
            LbError::Jwt("test".to_string()).error_type(),
            "authentication_error"
        );

        // Authorization errors
        assert_eq!(
            LbError::Authorization("test".to_string()).error_type(),
            "permission_error"
        );

        // Not found errors
        assert_eq!(
            LbError::NodeNotFound(Uuid::new_v4()).error_type(),
            "not_found_error"
        );
        assert_eq!(
            LbError::NotFound("test".to_string()).error_type(),
            "not_found_error"
        );
        assert_eq!(
            LbError::NoCapableNodes("test".to_string()).error_type(),
            "not_found_error"
        );

        // Service unavailable errors
        assert_eq!(
            LbError::NoNodesAvailable.error_type(),
            "service_unavailable"
        );
        assert_eq!(
            LbError::Http("test".to_string()).error_type(),
            "service_unavailable"
        );
        assert_eq!(
            LbError::NodeOffline(Uuid::new_v4()).error_type(),
            "service_unavailable"
        );

        // Server errors
        assert_eq!(
            LbError::Database("test".to_string()).error_type(),
            "server_error"
        );
        assert_eq!(
            LbError::Internal("test".to_string()).error_type(),
            "server_error"
        );

        // Invalid request errors
        assert_eq!(
            LbError::InvalidModelName("test".to_string()).error_type(),
            "invalid_request_error"
        );
    }

    #[test]
    fn test_lb_error_status_code() {
        assert_eq!(
            LbError::Authentication("test".to_string()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            LbError::Authorization("test".to_string()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            LbError::NodeNotFound(Uuid::new_v4()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            LbError::NotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            LbError::NoNodesAvailable.status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            LbError::Http("test".to_string()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            LbError::Timeout("test".to_string()).status_code(),
            StatusCode::GATEWAY_TIMEOUT
        );
        assert_eq!(
            LbError::Internal("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_lb_error_to_openai_error() {
        let error = LbError::NoNodesAvailable;
        let response = error.to_openai_error();

        assert_eq!(response.error.message, "No available runtimes");
        assert_eq!(response.error.error_type, "service_unavailable");
        assert_eq!(response.error.code, Some("503".to_string()));
    }

    #[test]
    fn test_openai_error_response_serialization() {
        let response = OpenAIErrorResponse {
            error: OpenAIErrorDetail {
                message: "Test error".to_string(),
                error_type: "invalid_request_error".to_string(),
                code: Some("400".to_string()),
            },
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"message\":\"Test error\""));
        assert!(json.contains("\"type\":\"invalid_request_error\""));
        assert!(json.contains("\"code\":\"400\""));
    }
}
