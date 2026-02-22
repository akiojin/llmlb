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
