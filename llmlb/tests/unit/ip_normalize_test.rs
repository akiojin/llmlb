// SPEC-62ac4b68 T001: IP正規化関数のユニットテスト（RED）

#[cfg(test)]
mod ip_normalize_tests {
    use llmlb::common::ip::normalize_ip;
    use llmlb::common::ip::normalize_socket_ip;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn test_ipv4_passthrough() {
        let ip: IpAddr = "192.168.1.100".parse().unwrap();
        let result = normalize_ip(ip);
        assert_eq!(result, ip);
    }

    #[test]
    fn test_ipv4_loopback_passthrough() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let result = normalize_ip(ip);
        assert_eq!(result, ip);
    }

    #[test]
    fn test_ipv6_passthrough() {
        let ip: IpAddr = "2001:db8:85a3::8a2e:370:7334".parse().unwrap();
        let result = normalize_ip(ip);
        assert_eq!(result, ip);
    }

    #[test]
    fn test_ipv6_loopback_passthrough() {
        let ip: IpAddr = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let result = normalize_ip(ip);
        assert_eq!(result, ip);
    }

    #[test]
    fn test_ipv4_mapped_ipv6_to_ipv4() {
        // ::ffff:192.168.1.1 -> 192.168.1.1
        let ip: IpAddr = "::ffff:192.168.1.1".parse().unwrap();
        let result = normalize_ip(ip);
        assert_eq!(result, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn test_ipv4_mapped_ipv6_loopback_to_ipv4() {
        // ::ffff:127.0.0.1 -> 127.0.0.1
        let ip: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        let result = normalize_ip(ip);
        assert_eq!(result, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn test_normalize_socket_ip_v4() {
        let addr: SocketAddr = "192.168.1.100:8080".parse().unwrap();
        let result = normalize_socket_ip(&addr);
        assert_eq!(result, "192.168.1.100".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_normalize_socket_ip_v6() {
        let addr: SocketAddr = "[2001:db8::1]:8080".parse().unwrap();
        let result = normalize_socket_ip(&addr);
        assert_eq!(result, "2001:db8::1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_normalize_socket_ip_v4_mapped() {
        let addr: SocketAddr = "[::ffff:10.0.0.1]:3000".parse().unwrap();
        let result = normalize_socket_ip(&addr);
        assert_eq!(result, "10.0.0.1".parse::<IpAddr>().unwrap());
    }
}
