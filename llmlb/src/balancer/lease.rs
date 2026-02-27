//! リクエストリース管理
//!
//! リクエスト処理中のライフサイクルを管理する `RequestLease` を提供する。

use crate::common::error::RouterResult;
use std::time::Duration as StdDuration;
use uuid::Uuid;

use super::types::RequestOutcome;
use super::LoadManager;

/// リクエスト処理中のlease
///
/// `complete*` が呼ばれずに破棄された場合でも、Drop時にエラーとして
/// activeカウンタを減算することでカウンタ残留を防ぐ。
pub struct RequestLease {
    load_manager: Option<LoadManager>,
    endpoint_id: Uuid,
    started_at: std::time::Instant,
}

impl RequestLease {
    pub(crate) fn new(load_manager: LoadManager, endpoint_id: Uuid) -> Self {
        Self {
            load_manager: Some(load_manager),
            endpoint_id,
            started_at: std::time::Instant::now(),
        }
    }

    /// 紐づくエンドポイントIDを返す。
    pub fn endpoint_id(&self) -> Uuid {
        self.endpoint_id
    }

    /// lease開始からの経過時間を返す。
    pub fn elapsed(&self) -> StdDuration {
        self.started_at.elapsed()
    }

    /// リクエストを指定結果で明示的に完了する。
    pub async fn complete(
        mut self,
        outcome: RequestOutcome,
        duration: StdDuration,
    ) -> RouterResult<()> {
        let Some(load_manager) = self.load_manager.take() else {
            return Ok(());
        };
        load_manager
            .finish_request(self.endpoint_id, outcome, duration)
            .await
    }

    /// トークン使用量付きでリクエストを明示的に完了する。
    pub async fn complete_with_tokens(
        mut self,
        outcome: RequestOutcome,
        duration: StdDuration,
        token_usage: Option<crate::token::TokenUsage>,
    ) -> RouterResult<()> {
        let Some(load_manager) = self.load_manager.take() else {
            return Ok(());
        };
        load_manager
            .finish_request_with_tokens(self.endpoint_id, outcome, duration, token_usage)
            .await
    }
}

impl Drop for RequestLease {
    fn drop(&mut self) {
        let Some(load_manager) = self.load_manager.take() else {
            return;
        };

        let endpoint_id = self.endpoint_id;
        let duration = self.started_at.elapsed();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(err) = load_manager
                    .finish_request(endpoint_id, RequestOutcome::Error, duration)
                    .await
                {
                    tracing::warn!(
                        endpoint_id = %endpoint_id,
                        error = %err,
                        "Failed to auto-complete leaked request lease"
                    );
                }
            });
        } else {
            tracing::warn!(
                endpoint_id = %endpoint_id,
                "Request lease dropped without runtime; skipping auto-complete"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use uuid::Uuid;

    // --- RequestLease field accessor tests ---

    #[test]
    fn request_lease_endpoint_id_returns_correct_value() {
        let id = Uuid::new_v4();
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: id,
            started_at: std::time::Instant::now(),
        };
        assert_eq!(lease.endpoint_id(), id);
    }

    #[test]
    fn request_lease_endpoint_id_deterministic() {
        let id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: id,
            started_at: std::time::Instant::now(),
        };
        assert_eq!(lease.endpoint_id(), id);
        // Call again to verify determinism
        assert_eq!(lease.endpoint_id(), id);
    }

    #[test]
    fn request_lease_elapsed_is_nonnegative() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        assert!(lease.elapsed() >= StdDuration::ZERO);
    }

    #[test]
    fn request_lease_elapsed_increases_over_time() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        let e1 = lease.elapsed();
        // Busy wait briefly
        std::thread::sleep(StdDuration::from_millis(5));
        let e2 = lease.elapsed();
        assert!(e2 >= e1);
    }

    // --- Drop behavior: no load_manager (no-op) ---

    #[test]
    fn request_lease_drop_without_load_manager_does_not_panic() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        drop(lease);
        // No panic means the test passes
    }

    // --- RequestOutcome tests ---

    #[test]
    fn request_outcome_debug_format() {
        let success = format!("{:?}", RequestOutcome::Success);
        let error = format!("{:?}", RequestOutcome::Error);
        let queued = format!("{:?}", RequestOutcome::Queued);
        assert_eq!(success, "Success");
        assert_eq!(error, "Error");
        assert_eq!(queued, "Queued");
    }

    #[test]
    fn request_outcome_clone() {
        let outcome = RequestOutcome::Success;
        let cloned = outcome;
        assert!(matches!(cloned, RequestOutcome::Success));
    }

    // --- Different endpoint IDs ---

    #[test]
    fn request_lease_nil_uuid() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::nil(),
            started_at: std::time::Instant::now(),
        };
        assert_eq!(
            lease.endpoint_id(),
            Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
        );
    }

    #[test]
    fn request_lease_max_uuid() {
        let id = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: id,
            started_at: std::time::Instant::now(),
        };
        assert_eq!(lease.endpoint_id(), id);
    }

    // --- Multiple leases can coexist ---

    #[test]
    fn multiple_leases_independent() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let lease1 = RequestLease {
            load_manager: None,
            endpoint_id: id1,
            started_at: std::time::Instant::now(),
        };
        let lease2 = RequestLease {
            load_manager: None,
            endpoint_id: id2,
            started_at: std::time::Instant::now(),
        };
        assert_ne!(lease1.endpoint_id(), lease2.endpoint_id());
    }

    // --- complete with None load_manager returns Ok ---

    #[tokio::test]
    async fn request_lease_complete_with_none_load_manager_returns_ok() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        let result = lease
            .complete(RequestOutcome::Success, StdDuration::from_millis(100))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn request_lease_complete_with_tokens_none_load_manager_returns_ok() {
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        let result = lease
            .complete_with_tokens(RequestOutcome::Error, StdDuration::from_millis(200), None)
            .await;
        assert!(result.is_ok());
    }

    // --- Started at is captured at construction time ---

    #[test]
    fn request_lease_started_at_is_early() {
        let before = std::time::Instant::now();
        let lease = RequestLease {
            load_manager: None,
            endpoint_id: Uuid::new_v4(),
            started_at: std::time::Instant::now(),
        };
        let after = std::time::Instant::now();
        // elapsed should be between 0 and (after - before)
        assert!(lease.elapsed() <= after.duration_since(before) + StdDuration::from_millis(1));
    }
}
