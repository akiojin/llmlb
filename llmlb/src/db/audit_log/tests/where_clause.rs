use super::*;

#[test]
fn test_build_where_clause_empty_filter() {
    let filter = AuditLogFilter::default();
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.is_empty());
    assert!(values.is_empty());
}

#[test]
fn test_build_where_clause_multiple_filters() {
    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        http_method: Some("GET".to_string()),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.starts_with("WHERE"));
    assert!(clause.contains("AND"));
    assert_eq!(values.len(), 2);
}

#[test]
fn test_build_extra_where_empty() {
    let result = build_extra_where("");
    assert!(result.is_empty());
}

#[test]
fn test_build_extra_where_with_conditions() {
    let result = build_extra_where("WHERE actor_type = ? AND http_method = ?");
    assert!(result.starts_with("AND"));
    assert!(result.contains("e.actor_type"));
    assert!(result.contains("e.http_method"));
}

// =====================================================================
// 追加テスト: build_where_clause single filter
// =====================================================================

#[test]
fn test_build_where_clause_actor_id_only() {
    let filter = AuditLogFilter {
        actor_id: Some("user-123".to_string()),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.starts_with("WHERE"));
    assert!(clause.contains("actor_id"));
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], "user-123");
}

#[test]
fn test_build_where_clause_request_path_only() {
    let filter = AuditLogFilter {
        request_path: Some("/v1/models".to_string()),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.contains("request_path"));
    assert_eq!(values[0], "/v1/models");
}

#[test]
fn test_build_where_clause_status_code_only() {
    let filter = AuditLogFilter {
        status_code: Some(401),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.contains("status_code"));
    assert_eq!(values[0], "401");
}

#[test]
fn test_build_where_clause_time_range() {
    let now = Utc::now();
    let filter = AuditLogFilter {
        time_from: Some(now - chrono::Duration::hours(1)),
        time_to: Some(now),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.contains("timestamp >= ?"));
    assert!(clause.contains("timestamp <= ?"));
    assert_eq!(values.len(), 2);
}

#[test]
fn test_build_where_clause_all_filters() {
    let now = Utc::now();
    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        actor_id: Some("id-1".to_string()),
        http_method: Some("GET".to_string()),
        request_path: Some("/api".to_string()),
        status_code: Some(200),
        time_from: Some(now),
        time_to: Some(now),
        ..Default::default()
    };
    let (clause, values) = build_where_clause(&filter);
    assert!(clause.starts_with("WHERE"));
    assert_eq!(values.len(), 7);
}

// =====================================================================
// 追加テスト: build_extra_where prefix replacement
// =====================================================================

#[test]
fn test_build_extra_where_replaces_all_column_names() {
    let where_clause = "WHERE actor_type = ? AND actor_id = ? AND http_method = ? AND request_path = ? AND status_code = ? AND timestamp >= ?";
    let result = build_extra_where(where_clause);
    assert!(result.starts_with("AND"));
    assert!(result.contains("e.actor_type"));
    assert!(result.contains("e.actor_id"));
    assert!(result.contains("e.http_method"));
    assert!(result.contains("e.request_path"));
    assert!(result.contains("e.status_code"));
    assert!(result.contains("e.timestamp"));
}
