//! 日次ダイジェストの集約
//!
//! 登録されている全エンドポイントの現在の状態を DB から読む。

use crate::common::error::RouterResult;
use crate::types::endpoint::{Endpoint, EndpointStatus};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::SqlitePool;

/// ダイジェストに載る 1 エンドポイント分の状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointDigestEntry {
    /// 表示名
    pub name: String,
    /// ベース URL
    pub base_url: String,
    /// エンドポイントタイプ
    pub endpoint_type: &'static str,
    /// 現在の状態
    pub status: EndpointStatus,
    /// 最終確認時刻
    pub last_seen: Option<DateTime<Utc>>,
    /// 最後のエラーメッセージ
    pub last_error: Option<String>,
}

/// 日次ダイジェストの内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestReport {
    /// 集約時刻（サーバーのローカル時刻）
    pub generated_at: NaiveDateTime,
    /// 全登録エンドポイント（対応が必要な状態を先頭に、同じ状態は名前順）
    pub endpoints: Vec<EndpointDigestEntry>,
}

impl DigestReport {
    /// エンドポイント一覧からダイジェストを作る
    pub fn from_endpoints(generated_at: NaiveDateTime, endpoints: Vec<Endpoint>) -> Self {
        let _ = (generated_at, endpoints);
        todo!("SPEC #777 T-005")
    }

    /// 指定した状態のエンドポイント数
    pub fn count(&self, status: EndpointStatus) -> usize {
        self.endpoints
            .iter()
            .filter(|endpoint| endpoint.status == status)
            .count()
    }

    /// 全登録エンドポイントの状態を DB から集約する
    pub async fn collect(pool: &SqlitePool, generated_at: NaiveDateTime) -> RouterResult<Self> {
        let _ = (pool, generated_at);
        todo!("SPEC #777 T-005")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::endpoint::EndpointType;

    fn generated_at() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-10-01 09:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn endpoint(name: &str, status: EndpointStatus) -> Endpoint {
        let mut endpoint = Endpoint::new(
            name.to_string(),
            format!("http://{name}.example:8080"),
            EndpointType::Xllm,
        );
        endpoint.status = status;
        endpoint
    }

    #[test]
    fn lists_endpoints_needing_attention_first_then_by_name() {
        let report = DigestReport::from_endpoints(
            generated_at(),
            vec![
                endpoint("zeta", EndpointStatus::Online),
                endpoint("alpha", EndpointStatus::Online),
                endpoint("pending-one", EndpointStatus::Pending),
                endpoint("broken", EndpointStatus::Error),
                endpoint("down-b", EndpointStatus::Offline),
                endpoint("down-a", EndpointStatus::Offline),
            ],
        );

        let names: Vec<&str> = report
            .endpoints
            .iter()
            .map(|endpoint| endpoint.name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["down-a", "down-b", "broken", "pending-one", "alpha", "zeta"]
        );
        assert_eq!(report.count(EndpointStatus::Online), 2);
        assert_eq!(report.count(EndpointStatus::Offline), 2);
        assert_eq!(report.count(EndpointStatus::Error), 1);
        assert_eq!(report.count(EndpointStatus::Pending), 1);
        assert_eq!(report.generated_at, generated_at());
    }

    #[test]
    fn carries_the_fields_shown_in_the_digest() {
        let mut source = endpoint("gpu", EndpointStatus::Offline);
        source.endpoint_type = EndpointType::Ollama;
        source.last_error = Some("connection refused".to_string());
        source.api_key = Some("sk-secret".to_string());

        let report = DigestReport::from_endpoints(generated_at(), vec![source.clone()]);
        assert_eq!(
            report.endpoints,
            vec![EndpointDigestEntry {
                name: "gpu".to_string(),
                base_url: "http://gpu.example:8080".to_string(),
                endpoint_type: "ollama",
                status: EndpointStatus::Offline,
                last_seen: source.last_seen,
                last_error: Some("connection refused".to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn collects_every_registered_endpoint_from_the_database() {
        let pool = crate::db::test_utils::test_db_pool().await;
        assert!(DigestReport::collect(&pool, generated_at())
            .await
            .unwrap()
            .endpoints
            .is_empty());

        for (name, status) in [
            ("online", EndpointStatus::Online),
            ("offline", EndpointStatus::Offline),
            ("error", EndpointStatus::Error),
            ("pending", EndpointStatus::Pending),
        ] {
            crate::db::endpoints::create_endpoint(&pool, &endpoint(name, status))
                .await
                .unwrap();
        }

        let report = DigestReport::collect(&pool, generated_at()).await.unwrap();
        let listed: Vec<(&str, EndpointStatus)> = report
            .endpoints
            .iter()
            .map(|endpoint| (endpoint.name.as_str(), endpoint.status))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("offline", EndpointStatus::Offline),
                ("error", EndpointStatus::Error),
                ("pending", EndpointStatus::Pending),
                ("online", EndpointStatus::Online),
            ]
        );
    }
}
