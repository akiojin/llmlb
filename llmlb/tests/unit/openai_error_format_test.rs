//! OpenAI互換エラーレスポンス形式テスト
//!
//! LbErrorからOpenAI互換エラーレスポンスへの変換をテスト

use axum::http::StatusCode;
use llmlb::common::error::{LbError, OpenAIErrorDetail, OpenAIErrorResponse};
use uuid::Uuid;

/// OpenAIエラーレスポンスのJSON形式が正しいことを確認
#[test]
fn test_openai_error_response_json_format() {
    let error = LbError::NoEndpointsAvailable;
    let response = error.to_openai_error();

    let json = serde_json::to_value(&response).expect("Failed to serialize");

    // OpenAI形式: { "error": { "message": ..., "type": ..., "code": ... } }
    assert!(
        json.get("error").is_some(),
        "Response should have 'error' field"
    );

    let error_obj = json.get("error").unwrap();
    assert!(
        error_obj.get("message").is_some(),
        "Error should have 'message' field"
    );
    assert!(
        error_obj.get("type").is_some(),
        "Error should have 'type' field"
    );
    // code is optional but should be present for our errors
    assert!(
        error_obj.get("code").is_some(),
        "Error should have 'code' field"
    );
}

/// 各エラータイプが適切なHTTPステータスコードを返すことを確認
#[test]
fn test_error_status_codes() {
    // 400 Bad Request
    assert_eq!(
        LbError::InvalidModelName("test".to_string()).status_code(),
        StatusCode::BAD_REQUEST
    );

    // 401 Unauthorized
    assert_eq!(
        LbError::Authentication("test".to_string()).status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        LbError::Jwt("test".to_string()).status_code(),
        StatusCode::UNAUTHORIZED
    );

    // 403 Forbidden
    assert_eq!(
        LbError::Authorization("test".to_string()).status_code(),
        StatusCode::FORBIDDEN
    );

    // 404 Not Found
    assert_eq!(
        LbError::EndpointNotFound(Uuid::new_v4()).status_code(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        LbError::NoCapableEndpoints("model".to_string()).status_code(),
        StatusCode::NOT_FOUND
    );

    // 500 Internal Server Error
    assert_eq!(
        LbError::Internal("test".to_string()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        LbError::Database("test".to_string()).status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    // 502 Bad Gateway
    assert_eq!(
        LbError::Http("test".to_string()).status_code(),
        StatusCode::BAD_GATEWAY
    );

    // 503 Service Unavailable
    assert_eq!(
        LbError::NoEndpointsAvailable.status_code(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        LbError::ServiceUnavailable("test".to_string()).status_code(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        LbError::EndpointOffline(Uuid::new_v4()).status_code(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    // 504 Gateway Timeout
    assert_eq!(
        LbError::Timeout("test".to_string()).status_code(),
        StatusCode::GATEWAY_TIMEOUT
    );

    // 507 Insufficient Storage
    assert_eq!(
        LbError::InsufficientStorage("test".to_string()).status_code(),
        StatusCode::INSUFFICIENT_STORAGE
    );
}

/// 各エラータイプが適切なOpenAIエラータイプを返すことを確認
#[test]
fn test_error_types() {
    // invalid_request_error
    assert_eq!(
        LbError::InvalidModelName("test".to_string()).error_type(),
        "invalid_request_error"
    );

    // authentication_error
    assert_eq!(
        LbError::Authentication("test".to_string()).error_type(),
        "authentication_error"
    );
    assert_eq!(
        LbError::Jwt("test".to_string()).error_type(),
        "authentication_error"
    );
    assert_eq!(
        LbError::PasswordHash("test".to_string()).error_type(),
        "authentication_error"
    );

    // permission_error
    assert_eq!(
        LbError::Authorization("test".to_string()).error_type(),
        "permission_error"
    );

    // not_found_error
    assert_eq!(
        LbError::EndpointNotFound(Uuid::new_v4()).error_type(),
        "not_found_error"
    );
    assert_eq!(
        LbError::NoCapableEndpoints("model".to_string()).error_type(),
        "not_found_error"
    );

    // server_error
    assert_eq!(
        LbError::Internal("test".to_string()).error_type(),
        "server_error"
    );
    assert_eq!(
        LbError::Database("test".to_string()).error_type(),
        "server_error"
    );
    assert_eq!(
        LbError::Timeout("test".to_string()).error_type(),
        "server_error"
    );
    assert_eq!(
        LbError::InsufficientStorage("test".to_string()).error_type(),
        "server_error"
    );

    // service_unavailable
    assert_eq!(
        LbError::NoEndpointsAvailable.error_type(),
        "service_unavailable"
    );
    assert_eq!(
        LbError::ServiceUnavailable("test".to_string()).error_type(),
        "service_unavailable"
    );
    assert_eq!(
        LbError::Http("test".to_string()).error_type(),
        "service_unavailable"
    );
    assert_eq!(
        LbError::EndpointOffline(Uuid::new_v4()).error_type(),
        "service_unavailable"
    );
}

/// エラーメッセージが内部情報を漏洩しないことを確認
#[test]
fn test_external_message_does_not_leak_internal_info() {
    // 内部IPアドレスを含むエラー
    let error = LbError::Http("connection failed to 192.168.1.100:8080".to_string());
    let external = error.external_message();

    // 内部詳細がexternalメッセージに含まれないことを確認
    assert!(
        !external.contains("192.168.1.100"),
        "External message should not contain internal IP"
    );
    assert!(
        !external.contains("8080"),
        "External message should not contain internal port"
    );

    // 一般的なメッセージが返されることを確認
    assert_eq!(external, "Backend service unavailable");
}

/// OpenAIErrorResponseの構造が正しいことを確認
#[test]
fn test_openai_error_response_structure() {
    let response = OpenAIErrorResponse {
        error: OpenAIErrorDetail {
            message: "Test error message".to_string(),
            error_type: "invalid_request_error".to_string(),
            code: Some("400".to_string()),
        },
    };

    let json_str = serde_json::to_string(&response).expect("Failed to serialize");

    // JSON文字列に必要なフィールドが含まれることを確認
    assert!(json_str.contains("\"error\""));
    assert!(json_str.contains("\"message\""));
    assert!(json_str.contains("\"type\""));
    assert!(json_str.contains("\"code\""));
    assert!(json_str.contains("Test error message"));
    assert!(json_str.contains("invalid_request_error"));
    assert!(json_str.contains("400"));
}

/// codeフィールドがNoneの場合はJSONに含まれないことを確認
#[test]
fn test_openai_error_response_without_code() {
    let response = OpenAIErrorResponse {
        error: OpenAIErrorDetail {
            message: "Test error".to_string(),
            error_type: "server_error".to_string(),
            code: None,
        },
    };

    let json_str = serde_json::to_string(&response).expect("Failed to serialize");

    // codeフィールドが含まれないことを確認
    assert!(!json_str.contains("\"code\""));
}

/// to_openai_error()が正しい値を生成することを確認
#[test]
fn test_to_openai_error_values() {
    let test_cases = vec![
        (
            LbError::NoEndpointsAvailable,
            "No available endpoints",
            "service_unavailable",
            "503",
        ),
        (
            LbError::Authentication("bad credentials".to_string()),
            "Authentication failed",
            "authentication_error",
            "401",
        ),
        (
            LbError::Authorization("not allowed".to_string()),
            "Access denied",
            "permission_error",
            "403",
        ),
        (
            LbError::InvalidModelName("bad-model".to_string()),
            "Invalid model name",
            "invalid_request_error",
            "400",
        ),
        (
            LbError::Internal("panic".to_string()),
            "Internal server error",
            "server_error",
            "500",
        ),
    ];

    for (error, expected_message, expected_type, expected_code) in test_cases {
        let response = error.to_openai_error();

        assert_eq!(
            response.error.message, expected_message,
            "Message mismatch for {:?}",
            error
        );
        assert_eq!(
            response.error.error_type, expected_type,
            "Type mismatch for {:?}",
            error
        );
        assert_eq!(
            response.error.code,
            Some(expected_code.to_string()),
            "Code mismatch for {:?}",
            error
        );
    }
}
