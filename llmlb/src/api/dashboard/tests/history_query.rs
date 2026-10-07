// ===== RequestHistoryQuery tests =====

#[test]
fn test_normalized_per_page_valid_sizes() {
    use super::RequestHistoryQuery;

    for &size in super::ALLOWED_PAGE_SIZES {
        let query = RequestHistoryQuery {
            page: 1,
            per_page: size,
            limit: None,
            offset: None,
            model: None,
            endpoint_id: None,
            status: None,
            start_time: None,
            end_time: None,
            client_ip: None,
        };
        assert_eq!(query.normalized_per_page(), size);
    }
}

#[test]
fn test_normalized_per_page_invalid_falls_back_to_default() {
    use super::{RequestHistoryQuery, DEFAULT_PAGE_SIZE};

    let query = RequestHistoryQuery {
        page: 1,
        per_page: 37,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    assert_eq!(query.normalized_per_page(), DEFAULT_PAGE_SIZE);
}

#[test]
fn test_normalized_per_page_zero_falls_back() {
    use super::{RequestHistoryQuery, DEFAULT_PAGE_SIZE};

    let query = RequestHistoryQuery {
        page: 1,
        per_page: 0,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    assert_eq!(query.normalized_per_page(), DEFAULT_PAGE_SIZE);
}

// ===== to_record_filter tests =====

#[test]
fn test_to_record_filter_valid() {
    use super::RequestHistoryQuery;

    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: Some("llama".to_string()),
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    let filter = query.to_record_filter().unwrap();
    assert_eq!(filter.model, Some("llama".to_string()));
}

#[test]
fn test_to_record_filter_start_after_end_error() {
    use super::RequestHistoryQuery;
    use chrono::TimeZone;

    let start = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap();
    let end = chrono::Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap();

    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: Some(start),
        end_time: Some(end),
        client_ip: None,
    };
    assert!(query.to_record_filter().is_err());
}

// ===== RequestHistoryExportQuery to_record_filter tests =====

#[test]
fn test_export_query_to_record_filter_valid() {
    use super::RequestHistoryExportQuery;

    let query = RequestHistoryExportQuery {
        format: super::RequestHistoryExportFormat::Json,
        model: Some("gpt-4".to_string()),
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    let filter = query.to_record_filter().unwrap();
    assert_eq!(filter.model, Some("gpt-4".to_string()));
}

#[test]
fn test_export_query_to_record_filter_start_after_end_error() {
    use super::RequestHistoryExportQuery;
    use chrono::TimeZone;

    let start = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
    let end = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    let query = RequestHistoryExportQuery {
        format: super::RequestHistoryExportFormat::Csv,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: Some(start),
        end_time: Some(end),
        client_ip: None,
    };
    assert!(query.to_record_filter().is_err());
}

// ===== RequestHistoryExportFormat tests =====

#[test]
fn test_export_format_default_is_csv() {
    use super::RequestHistoryExportFormat;
    assert_eq!(
        RequestHistoryExportFormat::default(),
        RequestHistoryExportFormat::Csv
    );
}

#[test]
fn test_export_format_deserialization() {
    use super::RequestHistoryExportFormat;
    assert_eq!(
        serde_json::from_str::<RequestHistoryExportFormat>("\"csv\"").unwrap(),
        RequestHistoryExportFormat::Csv
    );
    assert_eq!(
        serde_json::from_str::<RequestHistoryExportFormat>("\"json\"").unwrap(),
        RequestHistoryExportFormat::Json
    );
}

// ===== default functions tests =====

#[test]
fn test_default_page() {
    use super::default_page;
    assert_eq!(default_page(), 1);
}

#[test]
fn test_default_per_page() {
    use super::{default_per_page, DEFAULT_PAGE_SIZE};
    assert_eq!(default_per_page(), DEFAULT_PAGE_SIZE);
}

// ===== ALLOWED_PAGE_SIZES / DEFAULT_PAGE_SIZE constants tests =====

#[test]
fn test_allowed_page_sizes_are_reasonable() {
    use super::{ALLOWED_PAGE_SIZES, DEFAULT_PAGE_SIZE};
    assert!(ALLOWED_PAGE_SIZES.contains(&10));
    assert!(ALLOWED_PAGE_SIZES.contains(&25));
    assert!(ALLOWED_PAGE_SIZES.contains(&50));
    assert!(ALLOWED_PAGE_SIZES.contains(&100));
    assert!(ALLOWED_PAGE_SIZES.contains(&DEFAULT_PAGE_SIZE));
}

// ===== RequestHistoryQuery deserialization =====

#[test]
fn test_request_history_query_deserialization_defaults() {
    use super::RequestHistoryQuery;
    let q: RequestHistoryQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q.page, 1);
    assert_eq!(q.per_page, 10);
    assert!(q.model.is_none());
    assert!(q.endpoint_id.is_none());
    assert!(q.status.is_none());
    assert!(q.limit.is_none());
    assert!(q.offset.is_none());
}

#[test]
fn test_request_history_query_with_filters() {
    use super::RequestHistoryQuery;
    let json = r#"{"page": 2, "per_page": 25, "model": "llama"}"#;
    let q: RequestHistoryQuery = serde_json::from_str(json).unwrap();
    assert_eq!(q.page, 2);
    assert_eq!(q.per_page, 25);
    assert_eq!(q.model, Some("llama".to_string()));
}

// ===== RequestHistoryExportQuery deserialization =====

#[test]
fn test_export_query_deserialization() {
    use super::RequestHistoryExportQuery;
    let q: RequestHistoryExportQuery =
        serde_json::from_str(r#"{"format": "json", "model": "gpt-4"}"#).unwrap();
    assert_eq!(q.format, super::RequestHistoryExportFormat::Json);
    assert_eq!(q.model, Some("gpt-4".to_string()));
}

#[test]
fn test_export_query_default_format() {
    use super::RequestHistoryExportQuery;
    let q: RequestHistoryExportQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q.format, super::RequestHistoryExportFormat::Csv);
}

// --- RequestHistoryQuery extended tests ---

#[test]
fn test_normalized_per_page_large_value_falls_back() {
    use super::{RequestHistoryQuery, DEFAULT_PAGE_SIZE};
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 500,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    assert_eq!(query.normalized_per_page(), DEFAULT_PAGE_SIZE);
}

#[test]
fn test_normalized_per_page_one_falls_back() {
    use super::{RequestHistoryQuery, DEFAULT_PAGE_SIZE};
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 1,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    assert_eq!(query.normalized_per_page(), DEFAULT_PAGE_SIZE);
}

#[test]
fn test_to_record_filter_all_none() {
    use super::RequestHistoryQuery;
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    let filter = query.to_record_filter().unwrap();
    assert!(filter.model.is_none());
    assert!(filter.endpoint_id.is_none());
    assert!(filter.status.is_none());
    assert!(filter.start_time.is_none());
    assert!(filter.end_time.is_none());
    assert!(filter.client_ip.is_none());
}

#[test]
fn test_to_record_filter_with_client_ip() {
    use super::RequestHistoryQuery;
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: Some("192.168.1.1".to_string()),
    };
    let filter = query.to_record_filter().unwrap();
    assert_eq!(filter.client_ip, Some("192.168.1.1".to_string()));
}

#[test]
fn test_to_record_filter_with_endpoint_id() {
    use super::RequestHistoryQuery;
    let id = uuid::Uuid::new_v4();
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: Some(id),
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    let filter = query.to_record_filter().unwrap();
    assert_eq!(filter.endpoint_id, Some(id));
}

#[test]
fn test_to_record_filter_start_equals_end_is_ok() {
    use super::RequestHistoryQuery;
    use chrono::TimeZone;

    let ts = chrono::Utc.with_ymd_and_hms(2026, 2, 27, 12, 0, 0).unwrap();
    let query = RequestHistoryQuery {
        page: 1,
        per_page: 10,
        limit: None,
        offset: None,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: Some(ts),
        end_time: Some(ts),
        client_ip: None,
    };
    // start == end should be ok
    assert!(query.to_record_filter().is_ok());
}

// --- RequestHistoryExportQuery extended tests ---

#[test]
fn test_export_query_to_record_filter_all_none() {
    use super::RequestHistoryExportQuery;
    let query = RequestHistoryExportQuery {
        format: super::RequestHistoryExportFormat::Csv,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: None,
        end_time: None,
        client_ip: None,
    };
    let filter = query.to_record_filter().unwrap();
    assert!(filter.model.is_none());
    assert!(filter.endpoint_id.is_none());
}

#[test]
fn test_export_query_to_record_filter_start_equals_end_is_ok() {
    use super::RequestHistoryExportQuery;
    use chrono::TimeZone;

    let ts = chrono::Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
    let query = RequestHistoryExportQuery {
        format: super::RequestHistoryExportFormat::Json,
        model: None,
        endpoint_id: None,
        status: None,
        start_time: Some(ts),
        end_time: Some(ts),
        client_ip: None,
    };
    assert!(query.to_record_filter().is_ok());
}

// --- RequestHistoryExportFormat extended tests ---

#[test]
fn test_export_format_equality() {
    use super::RequestHistoryExportFormat;
    assert_eq!(
        RequestHistoryExportFormat::Csv,
        RequestHistoryExportFormat::Csv
    );
    assert_eq!(
        RequestHistoryExportFormat::Json,
        RequestHistoryExportFormat::Json
    );
    assert_ne!(
        RequestHistoryExportFormat::Csv,
        RequestHistoryExportFormat::Json
    );
}

#[test]
fn test_export_format_copy() {
    use super::RequestHistoryExportFormat;
    let fmt = RequestHistoryExportFormat::Json;
    let copied = fmt;
    assert_eq!(fmt, copied);
}

#[test]
fn test_export_format_debug() {
    use super::RequestHistoryExportFormat;
    let debug = format!("{:?}", RequestHistoryExportFormat::Csv);
    assert!(debug.contains("Csv"));
}

#[test]
fn test_export_format_invalid_deserialization() {
    use super::RequestHistoryExportFormat;
    assert!(serde_json::from_str::<RequestHistoryExportFormat>("\"xml\"").is_err());
    assert!(serde_json::from_str::<RequestHistoryExportFormat>("\"CSV\"").is_err());
}

// --- RequestHistoryQuery deserialization extended tests ---

#[test]
fn test_request_history_query_with_limit_and_offset() {
    use super::RequestHistoryQuery;
    let json = r#"{"limit": 50, "offset": 100}"#;
    let q: RequestHistoryQuery = serde_json::from_str(json).unwrap();
    assert_eq!(q.limit, Some(50));
    assert_eq!(q.offset, Some(100));
}

#[test]
fn test_request_history_query_with_endpoint_id_alias() {
    use super::RequestHistoryQuery;
    let id = uuid::Uuid::new_v4();
    let json = format!(r#"{{"node_id": "{}"}}"#, id);
    let q: RequestHistoryQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(q.endpoint_id, Some(id));
}
