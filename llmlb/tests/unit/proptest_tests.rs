//! Property-based tests using proptest

use proptest::prelude::*;
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use llmlb::api::model_name::parse_quantized_model_name;
use llmlb::api::openai_util::sanitize_openai_payload_for_history;
use llmlb::balancer::types::ModelTpsState;
use llmlb::common::auth::{ApiKeyPermission, UserRole};
use llmlb::common::ip::normalize_ip;

// ---------------------------------------------------------------------------
// ModelTpsState::update_tps (~6 tests)
// ---------------------------------------------------------------------------

proptest! {
    /// 任意の(output_tokens, duration_ms)でEMAが非負
    #[test]
    fn tps_ema_is_non_negative(
        tokens in 0u64..10_000,
        dur in 0u64..100_000,
    ) {
        let mut state = ModelTpsState::default();
        state.update_tps(tokens, dur);
        if let Some(ema) = state.tps_ema {
            prop_assert!(ema >= 0.0, "EMA was negative: {}", ema);
        }
    }

    /// duration_ms > 0 なら常にtps_emaがSome
    #[test]
    fn tps_ema_is_some_when_duration_positive(
        tokens in 0u64..10_000,
        dur in 1u64..100_000,
    ) {
        let mut state = ModelTpsState::default();
        state.update_tps(tokens, dur);
        prop_assert!(state.tps_ema.is_some(), "tps_ema should be Some when duration > 0");
    }

    /// duration_ms == 0 なら更新されない
    #[test]
    fn tps_ema_unchanged_when_duration_zero(tokens in 0u64..10_000) {
        let mut state = ModelTpsState::default();
        state.update_tps(tokens, 0);
        prop_assert!(state.tps_ema.is_none(), "tps_ema should remain None when duration == 0");
        prop_assert_eq!(state.request_count, 0);
    }

    /// request_count は呼び出し回数と一致
    #[test]
    fn request_count_matches_call_count(
        calls in prop::collection::vec((0u64..1_000, 1u64..10_000), 1..20),
    ) {
        let mut state = ModelTpsState::default();
        for (tokens, dur) in &calls {
            state.update_tps(*tokens, *dur);
        }
        prop_assert_eq!(state.request_count, calls.len() as u64);
    }

    /// total_output_tokens は入力の合計
    #[test]
    fn total_output_tokens_is_sum(
        calls in prop::collection::vec((0u64..1_000, 1u64..10_000), 1..20),
    ) {
        let mut state = ModelTpsState::default();
        let expected: u64 = calls.iter().map(|(t, _)| t).sum();
        for (tokens, dur) in &calls {
            state.update_tps(*tokens, *dur);
        }
        prop_assert_eq!(state.total_output_tokens, expected);
    }

    /// total_duration_ms は入力の合計
    #[test]
    fn total_duration_ms_is_sum(
        calls in prop::collection::vec((0u64..1_000, 1u64..10_000), 1..20),
    ) {
        let mut state = ModelTpsState::default();
        let expected: u64 = calls.iter().map(|(_, d)| d).sum();
        for (tokens, dur) in &calls {
            state.update_tps(*tokens, *dur);
        }
        prop_assert_eq!(state.total_duration_ms, expected);
    }
}

// ---------------------------------------------------------------------------
// parse_quantized_model_name (~4 tests)
// ---------------------------------------------------------------------------

proptest! {
    /// 任意のString入力でパニックしない
    #[test]
    fn parse_model_name_no_panic(s in ".*") {
        let _ = parse_quantized_model_name(&s);
    }

    /// 有効な「base:quant」形式は常にSome(quant)を返す
    #[test]
    fn parse_model_name_valid_format_returns_quant(
        base in "[a-zA-Z0-9_/-]{1,30}",
        quant in "[a-zA-Z0-9_]{1,10}",
    ) {
        let input = format!("{base}:{quant}");
        let result = parse_quantized_model_name(&input).unwrap();
        prop_assert_eq!(result.quantization, Some(quant));
        prop_assert_eq!(result.base, base);
    }

    /// コロンなし入力はquantization=None
    #[test]
    fn parse_model_name_no_colon_gives_none(s in "[^:]{0,50}") {
        let result = parse_quantized_model_name(&s).unwrap();
        prop_assert!(result.quantization.is_none());
    }

    /// 結果のrawは常に入力と一致（成功時）
    #[test]
    fn parse_model_name_raw_matches_input(s in ".*") {
        if let Ok(result) = parse_quantized_model_name(&s) {
            prop_assert_eq!(&result.raw, &s);
        }
    }
}

// ---------------------------------------------------------------------------
// sanitize_openai_payload_for_history (~4 tests)
// ---------------------------------------------------------------------------

/// Strategy for simple JSON values (no arb_json, just string/number/bool/null)
fn simple_json_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| Value::Number(serde_json::Number::from(n))),
        "[a-zA-Z0-9 ]{0,50}".prop_map(Value::String),
    ]
}

proptest! {
    /// 任意のJSON Valueでパニックしない
    #[test]
    fn sanitize_payload_no_panic(val in simple_json_value()) {
        let _ = sanitize_openai_payload_for_history(&val);
    }

    /// 出力にdata: + ;base64, を含む文字列が残らない
    #[test]
    fn sanitize_payload_removes_base64_data_urls(
        prefix in "[a-z]{1,10}",
        base64_data in "[A-Za-z0-9+/=]{10,50}",
    ) {
        let data_url = format!("data:{prefix};base64,{base64_data}");
        let input = Value::String(data_url);
        let output = sanitize_openai_payload_for_history(&input);
        let out_str = output.as_str().unwrap();
        prop_assert!(!out_str.contains(";base64,"), "base64 data-url should be redacted");
    }

    /// 入力にbase64なし → 出力は入力と同一
    #[test]
    fn sanitize_payload_preserves_non_base64(s in "[a-zA-Z0-9 ]{0,50}") {
        let input = Value::String(s.clone());
        let output = sanitize_openai_payload_for_history(&input);
        prop_assert_eq!(output, Value::String(s));
    }

    /// null/bool/number は変化しない
    #[test]
    fn sanitize_payload_primitives_unchanged(val in prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| Value::Number(serde_json::Number::from(n))),
    ]) {
        let output = sanitize_openai_payload_for_history(&val);
        prop_assert_eq!(output, val);
    }
}

// ---------------------------------------------------------------------------
// normalize_ip (~4 tests)
// ---------------------------------------------------------------------------

/// Strategy for arbitrary IPv4 addresses
fn arb_ipv4() -> impl Strategy<Value = Ipv4Addr> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(a, b, c, d)| Ipv4Addr::new(a, b, c, d))
}

/// Strategy for arbitrary IPv6 addresses
fn arb_ipv6() -> impl Strategy<Value = Ipv6Addr> {
    (
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
        any::<u16>(),
    )
        .prop_map(|(a, b, c, d, e, f, g, h)| Ipv6Addr::new(a, b, c, d, e, f, g, h))
}

/// Strategy for arbitrary IpAddr
fn arb_ip() -> impl Strategy<Value = IpAddr> {
    prop_oneof![
        arb_ipv4().prop_map(IpAddr::V4),
        arb_ipv6().prop_map(IpAddr::V6),
    ]
}

proptest! {
    /// 冪等性: normalize(normalize(ip)) == normalize(ip)
    #[test]
    fn normalize_ip_idempotent(ip in arb_ip()) {
        let once = normalize_ip(ip);
        let twice = normalize_ip(once);
        prop_assert_eq!(once, twice);
    }

    /// IPv4入力は常にIPv4出力
    #[test]
    fn normalize_ipv4_stays_ipv4(v4 in arb_ipv4()) {
        let result = normalize_ip(IpAddr::V4(v4));
        prop_assert!(result.is_ipv4(), "IPv4 input should produce IPv4 output");
    }

    /// 出力は常に有効なIpAddr（パースチェック）
    #[test]
    fn normalize_ip_output_is_valid(ip in arb_ip()) {
        let result = normalize_ip(ip);
        let s = result.to_string();
        let parsed: IpAddr = s.parse().unwrap();
        prop_assert_eq!(result, parsed);
    }

    /// IPv4-mapped IPv6は常にIPv4に変換される
    #[test]
    fn normalize_ipv4_mapped_ipv6_becomes_ipv4(v4 in arb_ipv4()) {
        let mapped = v4.to_ipv6_mapped();
        let result = normalize_ip(IpAddr::V6(mapped));
        prop_assert!(result.is_ipv4(), "IPv4-mapped IPv6 should become IPv4");
        prop_assert_eq!(result, IpAddr::V4(v4));
    }
}

// ---------------------------------------------------------------------------
// UserRole serde roundtrip (~3 tests)
// ---------------------------------------------------------------------------

fn arb_user_role() -> impl Strategy<Value = UserRole> {
    prop_oneof![Just(UserRole::Admin), Just(UserRole::Viewer),]
}

proptest! {
    /// UserRole serialize → deserialize roundtrip
    #[test]
    fn user_role_serde_roundtrip(role in arb_user_role()) {
        let json = serde_json::to_string(&role).unwrap();
        let back: UserRole = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(role, back);
    }

    /// UserRole serialize produces valid JSON string
    #[test]
    fn user_role_serializes_to_known_string(role in arb_user_role()) {
        let json = serde_json::to_string(&role).unwrap();
        prop_assert!(
            json == r#""admin""# || json == r#""viewer""#,
            "Unexpected serialized value: {}",
            json,
        );
    }

    /// UserRole invalid string always fails
    #[test]
    fn user_role_invalid_string_fails(s in "[a-z]{1,20}") {
        // Skip valid variants
        if s == "admin" || s == "viewer" {
            return Ok(());
        }
        let json = format!(r#""{s}""#);
        let result = serde_json::from_str::<UserRole>(&json);
        prop_assert!(result.is_err());
    }
}

// ---------------------------------------------------------------------------
// ApiKeyPermission serde roundtrip (~3 tests)
// ---------------------------------------------------------------------------

fn arb_api_key_permission() -> impl Strategy<Value = ApiKeyPermission> {
    prop_oneof![
        Just(ApiKeyPermission::OpenaiInference),
        Just(ApiKeyPermission::OpenaiModelsRead),
        Just(ApiKeyPermission::EndpointsRead),
        Just(ApiKeyPermission::EndpointsManage),
        Just(ApiKeyPermission::ApiKeysManage),
        Just(ApiKeyPermission::UsersManage),
        Just(ApiKeyPermission::InvitationsManage),
        Just(ApiKeyPermission::ModelsManage),
        Just(ApiKeyPermission::RegistryRead),
        Just(ApiKeyPermission::LogsRead),
        Just(ApiKeyPermission::MetricsRead),
    ]
}

proptest! {
    /// ApiKeyPermission serialize → deserialize roundtrip
    #[test]
    fn api_key_permission_serde_roundtrip(perm in arb_api_key_permission()) {
        let json = serde_json::to_string(&perm).unwrap();
        let back: ApiKeyPermission = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(perm, back);
    }

    /// ApiKeyPermission serialized value contains a dot
    #[test]
    fn api_key_permission_serialized_contains_dot(perm in arb_api_key_permission()) {
        let json = serde_json::to_string(&perm).unwrap();
        prop_assert!(json.contains('.'), "Serialized permission should contain a dot: {}", json);
    }

    /// ApiKeyPermission invalid string always fails
    #[test]
    fn api_key_permission_invalid_string_fails(s in "[a-z]{1,10}") {
        let json = format!(r#""{s}""#);
        let result = serde_json::from_str::<ApiKeyPermission>(&json);
        prop_assert!(result.is_err());
    }
}
