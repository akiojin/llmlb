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
