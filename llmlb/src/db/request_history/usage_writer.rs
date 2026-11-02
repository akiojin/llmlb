//! Tracked, retrying terminal writes drained before the server exits.

use super::RequestHistoryStorage;
use crate::common::error::{LbError, RouterResult};
use crate::common::protocol::RequestResponseRecord;
use crate::common::usage::{UsageAttribution, UsageTokens};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use uuid::Uuid;

type CommitCallback = Box<dyn FnOnce() + Send>;

struct PendingWrite {
    record: RequestResponseRecord,
    attribution: UsageAttribution,
    tokens: UsageTokens,
    committed: Mutex<Option<CommitCallback>>,
}

#[derive(Default)]
struct PendingState {
    writes: HashMap<Uuid, Arc<PendingWrite>>,
    running: bool,
    failures: u64,
    active_tasks: usize,
}

/// Shared by the original storage and every read-scoped clone.
#[derive(Default)]
pub(super) struct UsageWriter {
    state: Mutex<PendingState>,
    changed: Notify,
    worker: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// A metadata reader must enqueue its terminal observation before releasing this
/// registration, so shutdown cannot pass between parsing and terminal enqueue.
pub(crate) struct UsageTaskGuard(Arc<UsageWriter>);

impl Drop for UsageTaskGuard {
    fn drop(&mut self) {
        {
            let mut state = self
                .0
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.active_tasks = state.active_tasks.saturating_sub(1);
        }
        self.0.changed.notify_waiters();
    }
}

impl RequestHistoryStorage {
    /// Register a response metadata task with the graceful-shutdown barrier.
    pub(crate) fn track_usage_task(&self) -> UsageTaskGuard {
        self.usage_writer
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active_tasks += 1;
        UsageTaskGuard(self.usage_writer.clone())
    }
    /// Recover abandoned durable admissions before serving a new process.
    pub async fn recover_interrupted_usage(&self) -> RouterResult<u64> {
        crate::db::usage::recover_interrupted(&self.pool).await
    }
    /// Keep the terminal observation until its transaction commits. Callbacks
    /// run only after commit, and a single tracked worker retries write failures.
    pub(crate) fn enqueue_usage_settlement(
        &self,
        record: RequestResponseRecord,
        attribution: UsageAttribution,
        tokens: UsageTokens,
        committed: impl FnOnce() + Send + 'static,
    ) {
        {
            let mut state = self
                .usage_writer
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state
                .writes
                .entry(attribution.request_id)
                .or_insert_with(|| {
                    Arc::new(PendingWrite {
                        record,
                        attribution,
                        tokens,
                        committed: Mutex::new(Some(Box::new(committed))),
                    })
                });
        }
        self.start_usage_writer();
    }

    fn start_usage_writer(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            // Admission is already durable. A graceful drain can start this
            // worker again; an abrupt runtime exit is recovered at next start.
            return;
        };
        {
            let mut state = self
                .usage_writer
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if state.running || state.writes.is_empty() {
                return;
            }
            state.running = true;
        }
        let storage = self.clone();
        let handle = runtime.spawn(async move {
            storage.run_usage_writer().await;
        });
        *self
            .usage_writer
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(handle);
    }

    async fn run_usage_writer(&self) {
        let mut retry_delay = std::time::Duration::from_millis(10);
        loop {
            let next = {
                let mut state = self
                    .usage_writer
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                match state.writes.values().next().cloned() {
                    Some(next) => next,
                    None => {
                        state.running = false;
                        state.failures = 0;
                        self.usage_writer.changed.notify_waiters();
                        return;
                    }
                }
            };
            match self
                .save_attributed_record(&next.record, &next.attribution, &next.tokens, true)
                .await
            {
                Ok(()) => {
                    let committed = next
                        .committed
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .take();
                    if let Some(committed) = committed {
                        committed();
                    }
                    self.usage_writer
                        .state
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .writes
                        .remove(&next.attribution.request_id);
                    self.usage_writer.changed.notify_waiters();
                    retry_delay = std::time::Duration::from_millis(10);
                }
                Err(error) => {
                    {
                        let mut state = self
                            .usage_writer
                            .state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        state.failures = state.failures.saturating_add(1);
                    }
                    self.usage_writer.changed.notify_waiters();
                    tracing::error!(request_id = %next.attribution.request_id, %error, "Usage settlement retained for retry");
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(1));
                }
            }
        }
    }

    /// Reject additional admissions while observed terminal data cannot commit.
    pub(crate) fn ensure_usage_writer_available(&self) -> RouterResult<()> {
        let state = self
            .usage_writer
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if state.failures > 0 && !state.writes.is_empty() {
            Err(LbError::ServiceUnavailable(
                "Usage settlement is temporarily unavailable".into(),
            ))
        } else {
            Ok(())
        }
    }

    /// Wait for all terminal transactions, including retries. Graceful shutdown
    /// deliberately retains the process if storage remains unavailable rather
    /// than discarding observations already received from providers.
    pub async fn drain_usage_writes(&self) {
        self.start_usage_writer();
        loop {
            let changed = self.usage_writer.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let finished = {
                let state = self
                    .usage_writer
                    .state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                state.writes.is_empty() && state.active_tasks == 0
            };
            if finished {
                break;
            }
            changed.await;
        }
        let worker = self
            .usage_writer
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(worker) = worker {
            if let Err(error) = worker.await {
                tracing::error!(%error, "Usage settlement worker did not finish cleanly");
            }
        }
    }

    #[cfg(test)]
    pub(crate) async fn wait_for_usage_write_failure(&self) {
        loop {
            let changed = self.usage_writer.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self
                .usage_writer
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .failures
                > 0
            {
                return;
            }
            changed.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::protocol::RequestType;
    use crate::common::usage::TokenSource;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn usage_terminal_write_failure_retries_and_drains() {
        let pool = crate::db::test_utils::test_db_pool().await;
        let storage = RequestHistoryStorage::new(pool.clone());
        let record = RequestResponseRecord::new(
            Uuid::new_v4(),
            "terminal retry fixture".into(),
            "127.0.0.1".parse().unwrap(),
            "retry-model".into(),
            RequestType::Chat,
            serde_json::json!({"model":"retry-model"}),
            axum::http::StatusCode::OK,
            std::time::Duration::from_millis(1),
            None,
            None,
        );
        let attribution = UsageAttribution {
            request_id: record.id,
            received_at: record.timestamp,
            organization_id: None,
            account_id: None,
            api_key_id: None,
            auth_source: "legacy".into(),
        };
        storage
            .save_attributed_record(&record, &attribution, &UsageTokens::default(), false)
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER fail_usage_terminal BEFORE UPDATE OF finalized ON usage_receipts WHEN NEW.finalized = 1 BEGIN SELECT RAISE(ABORT, 'terminal storage unavailable'); END")
            .execute(&pool).await.unwrap();
        let committed = Arc::new(AtomicBool::new(false));
        let committed_callback = committed.clone();
        let tokens = UsageTokens::from_usage(
            &crate::token::TokenUsage::new(Some(11), Some(7), Some(18)),
            TokenSource::Provider,
        );
        storage.enqueue_usage_settlement(record, attribution, tokens, move || {
            committed_callback.store(true, Ordering::SeqCst);
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            storage.wait_for_usage_write_failure(),
        )
        .await
        .unwrap();
        assert!(
            !committed.load(Ordering::SeqCst),
            "settled must mean committed, not merely enqueued"
        );
        assert!(
            storage.ensure_usage_writer_available().is_err(),
            "new admissions fail closed during failed settlement"
        );
        let pending: (i64, i64) =
            sqlx::query_as("SELECT request_count, missing_count FROM usage_daily")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            pending,
            (1, 1),
            "the durable admission survives a terminal transaction failure"
        );
        sqlx::query("DROP TRIGGER fail_usage_terminal")
            .execute(&pool)
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            storage.drain_usage_writes(),
        )
        .await
        .unwrap();
        assert!(committed.load(Ordering::SeqCst));
        storage.ensure_usage_writer_available().unwrap();
        let totals: (i64, i64, i64, i64) = sqlx::query_as("SELECT request_count, total_tokens, successful_requests, missing_count FROM usage_daily")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(totals, (1, 18, 1, 0));
        storage.drain_usage_writes().await;
    }
}
