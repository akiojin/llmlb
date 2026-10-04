use super::*;

// =====================================================================
// 追加テスト: AuditLogRow -> AuditLogEntry conversion
// =====================================================================

#[test]
fn test_audit_log_entry_try_from_row() {
    let row = AuditLogRow {
        id: 1,
        timestamp: "2024-06-15T12:00:00+00:00".to_string(),
        http_method: "GET".to_string(),
        request_path: "/v1/models".to_string(),
        status_code: 200,
        actor_type: "user".to_string(),
        actor_id: Some("user-1".to_string()),
        actor_username: Some("admin".to_string()),
        api_key_owner_id: None,
        client_ip: Some("127.0.0.1".to_string()),
        duration_ms: Some(42),
        input_tokens: Some(100),
        output_tokens: Some(50),
        total_tokens: Some(150),
        model_name: Some("gpt-4".to_string()),
        endpoint_id: Some("ep-1".to_string()),
        detail: Some("test detail".to_string()),
        batch_id: None,
        is_migrated: 0,
    };

    let entry = AuditLogEntry::try_from(row).unwrap();
    assert_eq!(entry.id, Some(1));
    assert_eq!(entry.http_method, "GET");
    assert_eq!(entry.request_path, "/v1/models");
    assert_eq!(entry.status_code, 200);
    assert_eq!(entry.actor_type, ActorType::User);
    assert_eq!(entry.actor_id, Some("user-1".to_string()));
    assert_eq!(entry.actor_username, Some("admin".to_string()));
    assert_eq!(entry.client_ip, Some("127.0.0.1".to_string()));
    assert_eq!(entry.duration_ms, Some(42));
    assert_eq!(entry.input_tokens, Some(100));
    assert_eq!(entry.output_tokens, Some(50));
    assert_eq!(entry.total_tokens, Some(150));
    assert_eq!(entry.model_name, Some("gpt-4".to_string()));
    assert!(!entry.is_migrated);
}

#[test]
fn test_audit_log_entry_try_from_row_migrated() {
    let row = AuditLogRow {
        id: 2,
        timestamp: "2024-06-15T12:00:00+00:00".to_string(),
        http_method: "POST".to_string(),
        request_path: "/v1/chat/completions".to_string(),
        status_code: 200,
        actor_type: "api_key".to_string(),
        actor_id: None,
        actor_username: None,
        api_key_owner_id: None,
        client_ip: None,
        duration_ms: None,
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        model_name: None,
        endpoint_id: None,
        detail: None,
        batch_id: Some(5),
        is_migrated: 1,
    };

    let entry = AuditLogEntry::try_from(row).unwrap();
    assert!(entry.is_migrated);
    assert_eq!(entry.batch_id, Some(5));
    assert!(entry.actor_id.is_none());
    assert!(entry.client_ip.is_none());
    assert!(entry.duration_ms.is_none());
}

#[test]
fn test_audit_log_entry_try_from_invalid_timestamp() {
    let row = AuditLogRow {
        id: 1,
        timestamp: "not-a-date".to_string(),
        http_method: "GET".to_string(),
        request_path: "/".to_string(),
        status_code: 200,
        actor_type: "user".to_string(),
        actor_id: None,
        actor_username: None,
        api_key_owner_id: None,
        client_ip: None,
        duration_ms: None,
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
        model_name: None,
        endpoint_id: None,
        detail: None,
        batch_id: None,
        is_migrated: 0,
    };

    let result = AuditLogEntry::try_from(row);
    assert!(result.is_err());
}

// =====================================================================
// 追加テスト: AuditBatchHashRow -> AuditBatchHash conversion
// =====================================================================

#[test]
fn test_audit_batch_hash_try_from_row() {
    let row = AuditBatchHashRow {
        id: 1,
        sequence_number: 42,
        batch_start: "2024-01-01T00:00:00+00:00".to_string(),
        batch_end: "2024-01-01T01:00:00+00:00".to_string(),
        record_count: 100,
        hash: "abcdef".to_string(),
        previous_hash: "000000".to_string(),
    };

    let batch = AuditBatchHash::try_from(row).unwrap();
    assert_eq!(batch.id, Some(1));
    assert_eq!(batch.sequence_number, 42);
    assert_eq!(batch.record_count, 100);
    assert_eq!(batch.hash, "abcdef");
    assert_eq!(batch.previous_hash, "000000");
}

#[test]
fn test_audit_batch_hash_try_from_invalid_date() {
    let row = AuditBatchHashRow {
        id: 1,
        sequence_number: 1,
        batch_start: "not-a-date".to_string(),
        batch_end: "2024-01-01T00:00:00+00:00".to_string(),
        record_count: 0,
        hash: "x".to_string(),
        previous_hash: "y".to_string(),
    };

    let result = AuditBatchHash::try_from(row);
    assert!(result.is_err());
}
