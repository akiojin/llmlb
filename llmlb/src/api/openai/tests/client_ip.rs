use super::*;

#[test]
fn parse_client_ip_from_forwarded_value_supports_bracketed_ipv6_with_port() {
    let parsed = parse_client_ip_from_forwarded_value("\"[2001:db8::7]:4711\"")
        .expect("must parse bracketed ipv6");
    assert_eq!(parsed, "2001:db8::7".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_from_headers_prefers_first_valid_x_forwarded_for() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("unknown, 203.0.113.10, 10.0.0.1"),
    );
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=198.51.100.20;proto=https"),
    );

    let parsed = extract_client_ip_from_headers(&headers).expect("must parse x-forwarded-for");
    assert_eq!(parsed, "203.0.113.10".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_from_headers_falls_back_to_forwarded() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=unknown;proto=https, for=\"[2001:db8::11]:8443\""),
    );

    let parsed = extract_client_ip_from_headers(&headers).expect("must parse forwarded");
    assert_eq!(parsed, "2001:db8::11".parse::<IpAddr>().unwrap());
}

// ===== parse_client_ip_from_forwarded_value extended tests =====

#[test]
fn parse_client_ip_plain_ipv4() {
    let ip = parse_client_ip_from_forwarded_value("203.0.113.50").unwrap();
    assert_eq!(ip, "203.0.113.50".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_ipv4_with_port() {
    let ip = parse_client_ip_from_forwarded_value("203.0.113.50:8080").unwrap();
    assert_eq!(ip, "203.0.113.50".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_quoted_value() {
    let ip = parse_client_ip_from_forwarded_value("\"10.0.0.1\"").unwrap();
    assert_eq!(ip, "10.0.0.1".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_unknown_returns_none() {
    assert!(parse_client_ip_from_forwarded_value("unknown").is_none());
    assert!(parse_client_ip_from_forwarded_value("UNKNOWN").is_none());
}

#[test]
fn parse_client_ip_obfuscated_returns_none() {
    assert!(parse_client_ip_from_forwarded_value("_secret").is_none());
}

#[test]
fn parse_client_ip_empty_returns_none() {
    assert!(parse_client_ip_from_forwarded_value("").is_none());
    assert!(parse_client_ip_from_forwarded_value("  ").is_none());
}

#[test]
fn parse_client_ip_bracketed_ipv6() {
    let ip = parse_client_ip_from_forwarded_value("[::1]").unwrap();
    assert_eq!(ip, "::1".parse::<IpAddr>().unwrap());
}

// ===== extract_client_ip_from_headers extended tests =====

#[test]
fn extract_client_ip_returns_none_with_no_headers() {
    let headers = HeaderMap::new();
    assert!(extract_client_ip_from_headers(&headers).is_none());
}

#[test]
fn extract_client_ip_x_forwarded_for_single_ip() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
    let ip = extract_client_ip_from_headers(&headers).unwrap();
    assert_eq!(ip, "10.0.0.1".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_x_forwarded_for_skips_unknown() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("unknown, unknown, 192.168.1.1"),
    );
    let ip = extract_client_ip_from_headers(&headers).unwrap();
    assert_eq!(ip, "192.168.1.1".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_forwarded_header_for_key() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=10.20.30.40;proto=https"),
    );
    let ip = extract_client_ip_from_headers(&headers).unwrap();
    assert_eq!(ip, "10.20.30.40".parse::<IpAddr>().unwrap());
}

// ===== UNSPECIFIED_IP constant test =====

#[test]
fn unspecified_ip_is_ipv4_unspecified() {
    use super::UNSPECIFIED_IP;
    assert_eq!(
        UNSPECIFIED_IP,
        std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
    );
    assert!(UNSPECIFIED_IP.is_unspecified());
}

// ===== extract_forwarded_for / extract_x_forwarded_for tests =====

#[test]
fn extract_x_forwarded_for_multiple_ips() {
    use super::extract_x_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("192.168.1.1, 10.0.0.2, 172.16.0.3"),
    );
    let ip = extract_x_forwarded_for(&headers).unwrap();
    assert_eq!(ip, "192.168.1.1".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_x_forwarded_for_missing_header_returns_none() {
    use super::extract_x_forwarded_for;
    let headers = HeaderMap::new();
    assert!(extract_x_forwarded_for(&headers).is_none());
}

#[test]
fn extract_forwarded_for_missing_header_returns_none() {
    use super::extract_forwarded_for;
    let headers = HeaderMap::new();
    assert!(extract_forwarded_for(&headers).is_none());
}

#[test]
fn extract_forwarded_for_multiple_entries() {
    use super::extract_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=unknown;proto=https, for=198.51.100.10;proto=http"),
    );
    let ip = extract_forwarded_for(&headers).unwrap();
    assert_eq!(ip, "198.51.100.10".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_forwarded_for_case_insensitive_key() {
    use super::extract_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("FOR=192.0.2.60;proto=https"),
    );
    let ip = extract_forwarded_for(&headers).unwrap();
    assert_eq!(ip, "192.0.2.60".parse::<IpAddr>().unwrap());
}

// ===== extract_client_info tests =====

#[test]
fn extract_client_info_with_forwarded_header() {
    use super::extract_client_info;
    use std::net::SocketAddr;

    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.50"));

    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &None);
    assert_eq!(
        client_ip.unwrap(),
        "203.0.113.50".parse::<IpAddr>().unwrap()
    );
    assert!(api_key_id.is_none());
}

#[test]
fn extract_client_info_without_forwarded_falls_back_to_socket() {
    use super::extract_client_info;
    use std::net::SocketAddr;

    let addr: SocketAddr = "10.0.0.5:9999".parse().unwrap();
    let headers = HeaderMap::new();

    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &None);
    assert_eq!(client_ip.unwrap(), "10.0.0.5".parse::<IpAddr>().unwrap());
    assert!(api_key_id.is_none());
}

// --- parse_client_ip_from_forwarded_value edge cases ---

#[test]
fn parse_client_ip_plain_ipv6() {
    let ip = parse_client_ip_from_forwarded_value("::1").unwrap();
    assert_eq!(ip, "::1".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_full_ipv6() {
    let ip = parse_client_ip_from_forwarded_value("2001:db8::1").unwrap();
    assert_eq!(ip, "2001:db8::1".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_garbage_returns_none() {
    assert!(parse_client_ip_from_forwarded_value("not-an-ip").is_none());
    assert!(parse_client_ip_from_forwarded_value("abc.def.ghi.jkl").is_none());
}

#[test]
fn parse_client_ip_ipv4_mapped_ipv6() {
    // ::ffff:192.168.1.1 is an IPv4-mapped IPv6 address
    let ip = parse_client_ip_from_forwarded_value("::ffff:192.168.1.1").unwrap();
    // normalize_ip should convert this to an IPv4 address
    assert!(ip.is_ipv4());
}

#[test]
fn parse_client_ip_quoted_ipv6() {
    let ip = parse_client_ip_from_forwarded_value("\"[2001:db8::1]\"").unwrap();
    assert_eq!(ip, "2001:db8::1".parse::<IpAddr>().unwrap());
}

#[test]
fn parse_client_ip_whitespace_only_returns_none() {
    assert!(parse_client_ip_from_forwarded_value("   ").is_none());
}

#[test]
fn parse_client_ip_underscore_prefix_returns_none() {
    // Obfuscated identifiers start with underscore per RFC
    assert!(parse_client_ip_from_forwarded_value("_hidden").is_none());
    assert!(parse_client_ip_from_forwarded_value("_obfuscated123").is_none());
}

// --- extract_x_forwarded_for edge cases ---

#[test]
fn extract_x_forwarded_for_all_unknown() {
    use super::extract_x_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("unknown, unknown"),
    );
    assert!(extract_x_forwarded_for(&headers).is_none());
}

#[test]
fn extract_x_forwarded_for_single_valid_ip() {
    use super::extract_x_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("172.16.0.1"));
    let ip = extract_x_forwarded_for(&headers).unwrap();
    assert_eq!(ip, "172.16.0.1".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_x_forwarded_for_with_ipv6() {
    use super::extract_x_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("2001:db8::1, 10.0.0.1"),
    );
    let ip = extract_x_forwarded_for(&headers).unwrap();
    assert_eq!(ip, "2001:db8::1".parse::<IpAddr>().unwrap());
}

// --- extract_forwarded_for edge cases ---

#[test]
fn extract_forwarded_for_no_for_key() {
    use super::extract_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("proto=https;host=example.com"),
    );
    assert!(extract_forwarded_for(&headers).is_none());
}

#[test]
fn extract_forwarded_for_empty_value() {
    use super::extract_forwarded_for;
    let mut headers = HeaderMap::new();
    headers.insert("forwarded", HeaderValue::from_static(""));
    assert!(extract_forwarded_for(&headers).is_none());
}

// --- extract_client_info with auth context ---

#[test]
fn extract_client_info_client_ip_is_always_some() {
    use super::extract_client_info;
    use std::net::SocketAddr;

    let addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let headers = HeaderMap::new();

    let (client_ip, _) = extract_client_info(&addr, &headers, &None);
    assert!(client_ip.is_some());
}

#[test]
fn extract_client_info_prefers_x_forwarded_for_over_socket() {
    use super::extract_client_info;
    use std::net::SocketAddr;

    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));

    let (client_ip, _) = extract_client_info(&addr, &headers, &None);
    assert_eq!(client_ip.unwrap(), "1.2.3.4".parse::<IpAddr>().unwrap());
}

// --- extract_client_ip_from_headers edge cases ---

#[test]
fn extract_client_ip_x_forwarded_for_takes_priority_over_forwarded() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("10.0.0.1"));
    headers.insert("forwarded", HeaderValue::from_static("for=192.168.1.1"));
    let ip = extract_client_ip_from_headers(&headers).unwrap();
    // x-forwarded-for should take priority
    assert_eq!(ip, "10.0.0.1".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_only_forwarded_header() {
    let mut headers = HeaderMap::new();
    headers.insert("forwarded", HeaderValue::from_static("for=172.16.0.100"));
    let ip = extract_client_ip_from_headers(&headers).unwrap();
    assert_eq!(ip, "172.16.0.100".parse::<IpAddr>().unwrap());
}

#[test]
fn extract_client_ip_both_headers_all_invalid() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("unknown, _obfuscated"),
    );
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=unknown;proto=https"),
    );
    assert!(extract_client_ip_from_headers(&headers).is_none());
}
