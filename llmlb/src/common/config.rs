//! 設定管理
//!
//! LbConfig, NodeConfig等の設定構造体

use serde::{Deserialize, Serialize};

/// load balancer設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LbConfig {
    /// ホストアドレス (デフォルト: "0.0.0.0")
    #[serde(default = "default_host")]
    pub host: String,

    /// ポート番号 (デフォルト: 32768)
    #[serde(default = "default_port")]
    pub port: u16,

    /// データベースURL (デフォルト: "sqlite://lb.db")
    #[serde(default = "default_database_url")]
    pub database_url: String,

    /// ヘルスチェック間隔（秒）(デフォルト: 30)
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u64,

    /// ノードタイムアウト（秒）(デフォルト: 60)
    #[serde(default = "default_node_timeout")]
    pub node_timeout_secs: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    32768
}

fn default_database_url() -> String {
    "sqlite://lb.db".to_string()
}

fn default_health_check_interval() -> u64 {
    30
}

fn default_node_timeout() -> u64 {
    60
}

impl Default for LbConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            database_url: default_database_url(),
            health_check_interval_secs: default_health_check_interval(),
            node_timeout_secs: default_node_timeout(),
        }
    }
}

/// Node設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// load balancerのURL (デフォルト: "http://localhost:32768")
    #[serde(default = "default_lb_url")]
    pub lb_url: String,

    /// ノードランタイムのURL (デフォルト: "http://localhost:32768")
    #[serde(rename = "runtime_url", default = "default_runtime_url")]
    pub runtime_url: String,

    /// ハートビート送信間隔（秒）(デフォルト: 10)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Windows起動時の自動起動 (デフォルト: false)
    #[serde(default)]
    pub auto_start: bool,
}

fn default_lb_url() -> String {
    "http://localhost:32768".to_string()
}

fn default_runtime_url() -> String {
    "http://localhost:32768".to_string()
}

fn default_heartbeat_interval() -> u64 {
    10
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            lb_url: default_lb_url(),
            runtime_url: default_runtime_url(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            auto_start: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lb_config_defaults() {
        let config = LbConfig::default();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 32768);
        assert_eq!(config.database_url, "sqlite://lb.db");
        assert_eq!(config.health_check_interval_secs, 30);
        assert_eq!(config.node_timeout_secs, 60);
    }

    #[test]
    fn test_node_config_defaults() {
        let config = NodeConfig::default();

        assert_eq!(config.lb_url, "http://localhost:32768");
        assert_eq!(config.runtime_url, "http://localhost:32768");
        assert_eq!(config.heartbeat_interval_secs, 10);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_lb_config_deserialization() {
        let json = r#"{"host":"127.0.0.1","port":9000}"#;
        let config: LbConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        // デフォルト値が適用される
        assert_eq!(config.database_url, "sqlite://lb.db");
    }

    #[test]
    fn test_node_config_deserialization() {
        let json = r#"{"lb_url":"http://192.168.1.10:32768","auto_start":true}"#;
        let config: NodeConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.lb_url, "http://192.168.1.10:32768");
        assert!(config.auto_start);
        // デフォルト値が適用される
        assert_eq!(config.runtime_url, "http://localhost:32768");
    }
}
