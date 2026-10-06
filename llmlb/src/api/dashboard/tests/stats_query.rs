#[test]
fn test_default_days() {
    use super::default_days;
    assert_eq!(default_days(), Some(30));
}

#[test]
fn test_default_months() {
    use super::default_months;
    assert_eq!(default_months(), Some(12));
}

#[test]
fn test_default_clients_per_page() {
    use super::default_clients_per_page;
    assert_eq!(default_clients_per_page(), 20);
}

// ===== DailyTokenStatsQuery deserialization tests =====

#[test]
fn test_daily_token_stats_query_deserialization() {
    use super::DailyTokenStatsQuery;
    let q: DailyTokenStatsQuery = serde_json::from_str(r#"{"days": 7}"#).unwrap();
    assert_eq!(q.days, Some(7));
}

#[test]
fn test_daily_token_stats_query_default_days() {
    use super::DailyTokenStatsQuery;
    let q: DailyTokenStatsQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q.days, Some(30));
}

// ===== MonthlyTokenStatsQuery deserialization tests =====

#[test]
fn test_monthly_token_stats_query_deserialization() {
    use super::MonthlyTokenStatsQuery;
    let q: MonthlyTokenStatsQuery = serde_json::from_str(r#"{"months": 6}"#).unwrap();
    assert_eq!(q.months, Some(6));
}

#[test]
fn test_monthly_token_stats_query_default_months() {
    use super::MonthlyTokenStatsQuery;
    let q: MonthlyTokenStatsQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q.months, Some(12));
}

// ===== DailyTokenStats serialization =====

#[test]
fn test_daily_token_stats_serialization() {
    use super::DailyTokenStats;
    let stats = DailyTokenStats {
        date: "2026-02-27".to_string(),
        total_input_tokens: 1000,
        total_output_tokens: 500,
        total_tokens: 1500,
        request_count: 10,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["date"], "2026-02-27");
    assert_eq!(json["total_input_tokens"], 1000);
    assert_eq!(json["total_output_tokens"], 500);
    assert_eq!(json["total_tokens"], 1500);
    assert_eq!(json["request_count"], 10);
}

// ===== MonthlyTokenStats serialization =====

#[test]
fn test_monthly_token_stats_serialization() {
    use super::MonthlyTokenStats;
    let stats = MonthlyTokenStats {
        month: "2026-02".to_string(),
        total_input_tokens: 50000,
        total_output_tokens: 25000,
        total_tokens: 75000,
        request_count: 500,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["month"], "2026-02");
    assert_eq!(json["total_input_tokens"], 50000);
    assert_eq!(json["total_output_tokens"], 25000);
    assert_eq!(json["total_tokens"], 75000);
    assert_eq!(json["request_count"], 500);
}

// ===== EndpointDailyStatsQuery deserialization =====

#[test]
fn test_endpoint_daily_stats_query_default() {
    use super::EndpointDailyStatsQuery;
    let q: EndpointDailyStatsQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert!(q.days.is_none());
}

#[test]
fn test_endpoint_daily_stats_query_with_days() {
    use super::EndpointDailyStatsQuery;
    let q: EndpointDailyStatsQuery = serde_json::from_str(r#"{"days": 14}"#).unwrap();
    assert_eq!(q.days, Some(14));
}

// ===== ClientsQuery deserialization =====

#[test]
fn test_clients_query_defaults() {
    use super::ClientsQuery;
    let q: ClientsQuery = serde_json::from_str(r#"{}"#).unwrap();
    assert_eq!(q.page, 1);
    assert_eq!(q.per_page, 20);
}

#[test]
fn test_clients_query_custom() {
    use super::ClientsQuery;
    let q: ClientsQuery = serde_json::from_str(r#"{"page": 3, "per_page": 50}"#).unwrap();
    assert_eq!(q.page, 3);
    assert_eq!(q.per_page, 50);
}

// --- DailyTokenStats extended tests ---

#[test]
fn test_daily_token_stats_clone() {
    use super::DailyTokenStats;
    let stats = DailyTokenStats {
        date: "2026-01-01".to_string(),
        total_input_tokens: 100,
        total_output_tokens: 50,
        total_tokens: 150,
        request_count: 5,
    };
    let cloned = stats.clone();
    assert_eq!(cloned.date, "2026-01-01");
    assert_eq!(cloned.request_count, 5);
}

// --- MonthlyTokenStats extended tests ---

#[test]
fn test_monthly_token_stats_clone() {
    use super::MonthlyTokenStats;
    let stats = MonthlyTokenStats {
        month: "2026-01".to_string(),
        total_input_tokens: 10000,
        total_output_tokens: 5000,
        total_tokens: 15000,
        request_count: 100,
    };
    let cloned = stats.clone();
    assert_eq!(cloned.month, "2026-01");
    assert_eq!(cloned.total_tokens, 15000);
}

// --- EndpointDailyStatsQuery extended tests ---

#[test]
fn test_endpoint_daily_stats_query_with_zero_days() {
    use super::EndpointDailyStatsQuery;
    let q: EndpointDailyStatsQuery = serde_json::from_str(r#"{"days": 0}"#).unwrap();
    assert_eq!(q.days, Some(0));
}

#[test]
fn test_endpoint_daily_stats_query_with_large_days() {
    use super::EndpointDailyStatsQuery;
    let q: EndpointDailyStatsQuery = serde_json::from_str(r#"{"days": 1000}"#).unwrap();
    assert_eq!(q.days, Some(1000));
}

// --- ClientsQuery extended tests ---

#[test]
fn test_clients_query_zero_values() {
    use super::ClientsQuery;
    let q: ClientsQuery = serde_json::from_str(r#"{"page": 0, "per_page": 0}"#).unwrap();
    assert_eq!(q.page, 0);
    assert_eq!(q.per_page, 0);
}
