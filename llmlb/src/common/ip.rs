//! IPアドレス正規化ユーティリティ
//!
//! IPv4-mapped IPv6アドレスをIPv4に正規化する

use std::net::{IpAddr, SocketAddr};

/// IPアドレスを正規化する
///
/// IPv4-mapped IPv6（::ffff:x.x.x.x）をIPv4に変換。
/// それ以外はそのまま返す。
pub fn normalize_ip(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                IpAddr::V4(v4)
            } else {
                IpAddr::V6(v6)
            }
        }
        v4 => v4,
    }
}

/// SocketAddrからIPアドレスを抽出し正規化する
pub fn normalize_socket_ip(addr: &SocketAddr) -> IpAddr {
    normalize_ip(addr.ip())
}

/// IPv6アドレスを/64プレフィックスの文字列に変換する
///
/// IPv4はそのまま返す。IPv6は上位64ビットを保持し下位64ビットをゼロにした
/// `2001:db8:1234:5678::/64` 形式の文字列を返す。
pub fn ipv6_to_prefix64(ip_str: &str) -> String {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => {
            let segments = v6.segments();
            let prefix = std::net::Ipv6Addr::new(
                segments[0],
                segments[1],
                segments[2],
                segments[3],
                0,
                0,
                0,
                0,
            );
            format!("{prefix}/64")
        }
        _ => ip_str.to_string(),
    }
}
