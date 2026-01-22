//! エラー型定義
//!
//! 統一エラー型（thiserror使用）

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
}
