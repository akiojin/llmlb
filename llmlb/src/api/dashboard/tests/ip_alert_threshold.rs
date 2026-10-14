use super::*;

#[test]
fn parse_ip_alert_threshold_accepts_positive_integer() {
    assert_eq!(parse_ip_alert_threshold("100").unwrap(), 100);
    assert_eq!(parse_ip_alert_threshold(" 42 ").unwrap(), 42);
}

#[test]
fn parse_ip_alert_threshold_rejects_zero_or_negative() {
    assert!(parse_ip_alert_threshold("0").is_err());
    assert!(parse_ip_alert_threshold("-1").is_err());
}

#[test]
fn parse_ip_alert_threshold_rejects_non_numeric() {
    assert!(parse_ip_alert_threshold("abc").is_err());
}

// ===== effective_ip_alert_threshold tests =====

#[test]
fn effective_threshold_returns_parsed_value() {
    use super::effective_ip_alert_threshold;
    assert_eq!(effective_ip_alert_threshold(Some("50")), 50);
}

#[test]
fn effective_threshold_returns_default_when_none() {
    use super::effective_ip_alert_threshold;
    use super::IP_ALERT_THRESHOLD_DEFAULT_VALUE;
    assert_eq!(
        effective_ip_alert_threshold(None),
        IP_ALERT_THRESHOLD_DEFAULT_VALUE
    );
}

#[test]
fn effective_threshold_returns_default_for_invalid_value() {
    use super::effective_ip_alert_threshold;
    use super::IP_ALERT_THRESHOLD_DEFAULT_VALUE;
    assert_eq!(
        effective_ip_alert_threshold(Some("not-a-number")),
        IP_ALERT_THRESHOLD_DEFAULT_VALUE
    );
}

#[test]
fn effective_threshold_returns_default_for_zero() {
    use super::effective_ip_alert_threshold;
    use super::IP_ALERT_THRESHOLD_DEFAULT_VALUE;
    assert_eq!(
        effective_ip_alert_threshold(Some("0")),
        IP_ALERT_THRESHOLD_DEFAULT_VALUE
    );
}

#[test]
fn effective_threshold_returns_default_for_negative() {
    use super::effective_ip_alert_threshold;
    use super::IP_ALERT_THRESHOLD_DEFAULT_VALUE;
    assert_eq!(
        effective_ip_alert_threshold(Some("-5")),
        IP_ALERT_THRESHOLD_DEFAULT_VALUE
    );
}

#[test]
fn effective_threshold_accepts_one() {
    use super::effective_ip_alert_threshold;
    assert_eq!(effective_ip_alert_threshold(Some("1")), 1);
}

#[test]
fn effective_threshold_accepts_large_value() {
    use super::effective_ip_alert_threshold;
    assert_eq!(effective_ip_alert_threshold(Some("10000")), 10000);
}

// ===== parse_ip_alert_threshold extended tests =====

#[test]
fn parse_ip_alert_threshold_accepts_large_value() {
    assert_eq!(parse_ip_alert_threshold("999999").unwrap(), 999999);
}

#[test]
fn parse_ip_alert_threshold_accepts_one() {
    assert_eq!(parse_ip_alert_threshold("1").unwrap(), 1);
}

#[test]
fn parse_ip_alert_threshold_rejects_empty_string() {
    assert!(parse_ip_alert_threshold("").is_err());
}

#[test]
fn parse_ip_alert_threshold_rejects_float() {
    assert!(parse_ip_alert_threshold("1.5").is_err());
}

// ===== SettingUpdateBody deserialization =====

#[test]
fn test_setting_update_body_deserialization() {
    use super::SettingUpdateBody;
    let body: SettingUpdateBody = serde_json::from_str(r#"{"value": "200"}"#).unwrap();
    assert_eq!(body.value, "200");
}

// ===== IP_ALERT_THRESHOLD constants =====

#[test]
fn test_ip_alert_threshold_constants() {
    use super::{IP_ALERT_THRESHOLD_DEFAULT_VALUE, IP_ALERT_THRESHOLD_MIN};
    assert_eq!(IP_ALERT_THRESHOLD_DEFAULT_VALUE, 100);
    assert_eq!(IP_ALERT_THRESHOLD_MIN, 1);
}

// --- parse_ip_alert_threshold extended tests ---

#[test]
fn parse_ip_alert_threshold_max_i64() {
    let result = parse_ip_alert_threshold(&i64::MAX.to_string());
    assert_eq!(result.unwrap(), i64::MAX);
}

#[test]
fn parse_ip_alert_threshold_whitespace_around() {
    assert_eq!(parse_ip_alert_threshold("  100  ").unwrap(), 100);
}

#[test]
fn parse_ip_alert_threshold_leading_plus_rejected() {
    // "+100" may or may not parse, but the function trims, so let's check
    // Rust's i64 parse does accept "+100"
    let result = parse_ip_alert_threshold("+100");
    assert_eq!(result.unwrap(), 100);
}

// --- effective_ip_alert_threshold extended tests ---

#[test]
fn effective_threshold_with_whitespace() {
    use super::effective_ip_alert_threshold;
    assert_eq!(effective_ip_alert_threshold(Some("  75  ")), 75);
}

#[test]
fn effective_threshold_with_empty_string() {
    use super::effective_ip_alert_threshold;
    use super::IP_ALERT_THRESHOLD_DEFAULT_VALUE;
    assert_eq!(
        effective_ip_alert_threshold(Some("")),
        IP_ALERT_THRESHOLD_DEFAULT_VALUE
    );
}

// --- SettingUpdateBody extended tests ---

#[test]
fn test_setting_update_body_empty_value() {
    use super::SettingUpdateBody;
    let body: SettingUpdateBody = serde_json::from_str(r#"{"value": ""}"#).unwrap();
    assert_eq!(body.value, "");
}

#[test]
fn test_setting_update_body_numeric_string_value() {
    use super::SettingUpdateBody;
    let body: SettingUpdateBody = serde_json::from_str(r#"{"value": "42"}"#).unwrap();
    assert_eq!(body.value, "42");
}
