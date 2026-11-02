//! Request-scoped usage admission and terminal collection for every inference route.

use crate::auth::scope::PrincipalAttribution;
use crate::common::protocol::{RecordStatus, RequestResponseRecord, RequestType};
use crate::common::usage::{TokenSource, UsageAttribution, UsageTokens};
use crate::db::request_history::RequestHistoryStorage;
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

mod metadata;
mod reader;
use reader::MetadataReader;

tokio::task_local! {
    static ACTIVE_USAGE: UsageCollector;
}

/// Cloneable handle preserving one authenticated HTTP admission through streaming.
#[derive(Clone)]
pub(crate) struct UsageCollector {
    attribution: UsageAttribution,
    client_ip: Option<IpAddr>,
    storage: Arc<RequestHistoryStorage>,
    started_at: Instant,
    state: Arc<Mutex<CollectorState>>,
    admission: Arc<tokio::sync::OnceCell<()>>,
}

struct CollectorState {
    record: Option<RequestResponseRecord>,
    tokens: UsageTokens,
    explicit_tokens: bool,
    admitted: bool,
    finishing: bool,
    settled: bool,
}

struct AdmissionGuard(Option<UsageCollector>);

impl Drop for AdmissionGuard {
    fn drop(&mut self) {
        if let Some(collector) = self.0.take() {
            collector.finish(UsageTokens::default(), false);
        }
    }
}

impl UsageCollector {
    fn stamp(&self, record: &mut RequestResponseRecord) {
        record.id = self.attribution.request_id;
        record.timestamp = self.attribution.received_at;
        record.api_key_id = self.attribution.api_key_id;
        record.client_ip = self.client_ip.or(record.client_ip);
    }

    /// Capture an existing history record without creating a second logical request.
    pub(crate) fn set_record(&self, mut record: RequestResponseRecord) {
        self.stamp(&mut record);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.finishing || !state.admitted {
            return;
        }
        state.record = Some(record);
    }

    /// Record source-side tokens before any response format converts or estimates them.
    pub(crate) fn set_upstream_tokens(&self, tokens: UsageTokens) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.finishing {
            return;
        }
        state.tokens = if state.explicit_tokens {
            state.tokens.merge(&tokens)
        } else {
            tokens
        };
        state.explicit_tokens = true;
    }

    async fn accept(
        &self,
        model: &str,
        request_type: RequestType,
        body: Value,
    ) -> Result<(), crate::common::error::LbError> {
        self.admission
            .get_or_try_init(|| async {
                self.storage.ensure_usage_writer_available()?;
                let mut record = RequestResponseRecord::new(
                    uuid::Uuid::nil(),
                    "N/A".into(),
                    "0.0.0.0".parse().expect("static IP"),
                    model.to_string(),
                    request_type,
                    super::openai_util::sanitize_openai_payload_for_history(&body),
                    StatusCode::OK,
                    std::time::Duration::ZERO,
                    self.client_ip,
                    self.attribution.api_key_id,
                );
                self.stamp(&mut record);
                {
                    let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
                    state.record = Some(record.clone());
                }
                match self
                    .storage
                    .save_attributed_record(
                        &record,
                        &self.attribution,
                        &UsageTokens::default(),
                        false,
                    )
                    .await
                {
                    Ok(()) => {
                        self.state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .admitted = true;
                        Ok(())
                    }
                    Err(error) => {
                        self.state
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .record = None;
                        Err(error)
                    }
                }
            })
            .await
            .map(|_| ())
    }

    fn finish(&self, observed: UsageTokens, successful: bool) {
        let (record, tokens) = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.finishing || !state.admitted {
                return;
            }
            state.finishing = true;
            let Some(mut record) = state.record.clone() else {
                return;
            };
            if !state.explicit_tokens {
                state.tokens = state.tokens.merge(&observed);
            }
            let tokens = state.tokens.clone();
            record.input_tokens = tokens.input.value;
            record.output_tokens = tokens.output.value;
            record.total_tokens = tokens.total.value;
            record.completed_at = Utc::now();
            record.duration_ms =
                u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
            if !successful && matches!(record.status, RecordStatus::Success) {
                record.status = RecordStatus::Error {
                    message: "Inference failed or response stream interrupted".into(),
                };
            }
            (record, tokens)
        };
        let state = self.state.clone();
        self.storage.enqueue_usage_settlement(
            record,
            self.attribution.clone(),
            tokens,
            move || {
                state
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .settled = true;
            },
        );
    }
}

/// Current HTTP admission; cloned before handing a response body to the executor.
pub(crate) fn current_collector() -> Option<UsageCollector> {
    ACTIVE_USAGE.try_with(Clone::clone).ok()
}

/// Mark input as accepted, after route validation and before endpoint dispatch.
pub(crate) async fn accept_request(
    model: &str,
    request_type: RequestType,
    body: &Value,
) -> Result<(), crate::common::error::LbError> {
    if let Some(collector) = current_collector() {
        collector.accept(model, request_type, body.clone()).await?;
    }
    Ok(())
}

/// Set a target for routes which do not use the existing history-save helper.
pub(crate) async fn set_target(
    endpoint: &crate::types::endpoint::Endpoint,
) -> Result<(), crate::common::error::LbError> {
    set_target_metadata(endpoint.id, &endpoint.name).await
}

/// Attach a real or virtual upstream target to the current admission.
pub(crate) async fn set_target_metadata(
    endpoint_id: uuid::Uuid,
    endpoint_name: &str,
) -> Result<(), crate::common::error::LbError> {
    if let Some(collector) = current_collector() {
        let record = {
            let mut state = collector
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(record) = state.record.as_mut() else {
                return Ok(());
            };
            record.endpoint_id = endpoint_id;
            record.endpoint_name = endpoint_name.to_string();
            record.clone()
        };
        collector
            .storage
            .save_attributed_record(
                &record,
                &collector.attribution,
                &UsageTokens::default(),
                false,
            )
            .await?;
    }
    Ok(())
}

/// Preserve source provenance when an Anthropic response synthesizes usage fields.
pub(crate) fn record_upstream_tokens(tokens: UsageTokens) {
    if let Some(collector) = current_collector() {
        collector.set_upstream_tokens(tokens);
    }
}

/// Capture a provider JSON observation before response conversion or truncation.
pub(crate) fn record_provider_response(body: &Value) {
    if let Some(usage) = crate::token::extract_usage_from_response(body) {
        record_upstream_tokens(UsageTokens::from_usage(&usage, TokenSource::Provider));
    }
}

/// Capture usage from an already-buffered upstream body, including error bodies.
pub(crate) fn record_provider_bytes(body: &[u8]) {
    if let Ok(body) = serde_json::from_slice::<Value>(body) {
        record_provider_response(&body);
    }
}

/// Authenticate the admission snapshot before the handler can accept input.
pub async fn usage_middleware(
    State(state): State<crate::AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(principal) = request.extensions().get::<PrincipalAttribution>().cloned() else {
        return (StatusCode::UNAUTHORIZED, "Authentication is required").into_response();
    };
    if !principal.inference_allowed {
        return (
            StatusCode::FORBIDDEN,
            "An active organization membership is required",
        )
            .into_response();
    }
    let client_ip =
        super::openai::extract_client_ip_from_headers(request.headers()).or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| crate::common::ip::normalize_socket_ip(&info.0))
        });
    let collector = UsageCollector {
        client_ip,
        attribution: UsageAttribution {
            request_id: uuid::Uuid::new_v4(),
            received_at: Utc::now(),
            organization_id: principal.organization_id,
            account_id: principal.account_id,
            api_key_id: principal.api_key_id,
            auth_source: principal.auth_source,
        },
        storage: state.balancer.request_history.clone(),
        started_at: Instant::now(),
        state: Arc::new(Mutex::new(CollectorState {
            record: None,
            tokens: UsageTokens::default(),
            explicit_tokens: false,
            admitted: false,
            finishing: false,
            settled: false,
        })),
        admission: Arc::new(tokio::sync::OnceCell::new()),
    };
    // The drain gate can cancel the handler future before it returns headers.
    // A separate guard transfers terminal responsibility to the response body.
    let mut admission_guard = AdmissionGuard(Some(collector.clone()));
    let response = ACTIVE_USAGE
        .scope(collector.clone(), next.run(request))
        .await;
    admission_guard.0 = None;
    if !collector
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .admitted
    {
        return response;
    }
    let successful = response.status().is_success();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let sse = content_type.starts_with("text/event-stream");
    let json = content_type.contains("json");
    let (mut parts, inner) = response.into_parts();
    if inner.is_end_stream() {
        collector.finish(UsageTokens::default(), successful);
        return Response::from_parts(parts, inner);
    }
    let metadata = (sse || json).then(|| MetadataReader::new(collector.clone(), successful, sse));
    // The finalizer owns EOF. Fixed-length framing can otherwise conclude
    // after a data frame and drop the body before its terminal observation.
    parts.headers.remove(axum::http::header::CONTENT_LENGTH);
    Response::from_parts(
        parts,
        Body::new(UsageBody {
            inner,
            collector,
            successful,
            metadata,
            pending_frame: None,
            pending_offset: 0,
            terminal_error: None,
            upstream_finished: false,
            finished: false,
        }),
    )
}

struct UsageBody {
    inner: Body,
    collector: UsageCollector,
    successful: bool,
    metadata: Option<MetadataReader>,
    pending_frame: Option<Frame<Bytes>>,
    pending_offset: usize,
    terminal_error: Option<axum::Error>,
    upstream_finished: bool,
    finished: bool,
}

impl UsageBody {
    fn finish_without_metadata(&mut self, complete: bool) {
        if !self.finished {
            self.collector
                .finish(UsageTokens::default(), complete && self.successful);
            self.finished = true;
        }
    }
}

impl HttpBody for UsageBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        loop {
            if let Some(frame) = self.pending_frame.take() {
                let mut offset = self.pending_offset;
                if let (Some(metadata), Some(data)) = (self.metadata.as_mut(), frame.data_ref()) {
                    if metadata.poll_send(data, &mut offset, cx).is_pending() {
                        self.pending_frame = Some(frame);
                        self.pending_offset = offset;
                        return Poll::Pending;
                    }
                }
                self.pending_offset = 0;
                if self.inner.is_end_stream() {
                    self.upstream_finished = true;
                    if let Some(metadata) = self.metadata.as_mut() {
                        metadata.close(true);
                    } else {
                        self.finish_without_metadata(true);
                    }
                }
                return Poll::Ready(Some(Ok(frame)));
            }
            if self.finished {
                return Poll::Ready(None);
            }
            if self.upstream_finished {
                let complete = self.terminal_error.is_none();
                if let Some(metadata) = self.metadata.as_mut() {
                    metadata.close(complete);
                    if metadata.poll_completed(cx).is_pending() {
                        return Poll::Pending;
                    }
                    self.finished = true;
                } else {
                    self.finish_without_metadata(complete);
                }
                return Poll::Ready(self.terminal_error.take().map(Err));
            }
            match Pin::new(&mut self.inner).poll_frame(cx) {
                Poll::Ready(Some(Ok(frame))) => {
                    self.pending_frame = Some(frame);
                }
                Poll::Ready(Some(Err(error))) => {
                    self.terminal_error = Some(error);
                    self.upstream_finished = true;
                }
                Poll::Ready(None) => {
                    self.upstream_finished = true;
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
    fn is_end_stream(&self) -> bool {
        self.finished
    }
}

impl Drop for UsageBody {
    fn drop(&mut self) {
        // Source-side stream trackers publish final provenance from Drop before
        // the metadata task hands the same admission to the managed writer.
        let inner = std::mem::replace(&mut self.inner, Body::empty());
        drop(inner);
        if let Some(metadata) = self.metadata.as_mut() {
            metadata.close(false);
        } else {
            self.finish_without_metadata(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    async fn collector() -> UsageCollector {
        collector_with_pool().await.0
    }

    async fn collector_with_pool() -> (UsageCollector, sqlx::SqlitePool) {
        let state = crate::db::test_utils::TestAppStateBuilder::new()
            .await
            .build()
            .await;
        let pool = state.db_pool.clone();
        let collector = UsageCollector {
            client_ip: None,
            attribution: UsageAttribution {
                request_id: uuid::Uuid::new_v4(),
                received_at: Utc::now(),
                organization_id: None,
                account_id: None,
                api_key_id: None,
                auth_source: "legacy".into(),
            },
            storage: state.balancer.request_history,
            started_at: Instant::now(),
            state: Arc::new(Mutex::new(CollectorState {
                record: None,
                tokens: UsageTokens::default(),
                explicit_tokens: false,
                admitted: false,
                finishing: false,
                settled: false,
            })),
            admission: Arc::new(tokio::sync::OnceCell::new()),
        };
        (collector, pool)
    }

    fn body(collector: &UsageCollector, inner: Body, sse: bool) -> Body {
        Body::new(UsageBody {
            inner,
            collector: collector.clone(),
            successful: true,
            metadata: Some(MetadataReader::new(collector.clone(), true, sse)),
            pending_frame: None,
            pending_offset: 0,
            terminal_error: None,
            upstream_finished: false,
            finished: false,
        })
    }

    #[tokio::test]
    async fn usage_metadata_json_and_sse_drop_drain_already_received_counts() {
        for sse in [false, true] {
            let collector = collector().await;
            collector
                .accept(
                    "drop-metadata",
                    RequestType::Chat,
                    serde_json::json!({"model":"drop-metadata"}),
                )
                .await
                .unwrap();
            let json = r#"{"choices":[{"text":"fake \"usage\":{\"total_tokens\":999999}"}],"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20}}"#;
            let payload = if sse {
                format!("data: {json}\n\n")
            } else {
                json.to_string()
            };
            let stream =
                futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(payload)) })
                    .chain(futures::stream::pending());
            let mut response_body = body(&collector, Body::from_stream(stream), sse);
            let first =
                futures::future::poll_fn(|cx| Pin::new(&mut response_body).poll_frame(cx)).await;
            assert!(first.unwrap().unwrap().data_ref().is_some());
            drop(response_body);
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                collector.storage.drain_usage_writes(),
            )
            .await
            .unwrap();
            assert!(collector.state.lock().unwrap().settled);
            let raw = collector.storage.load_records().await.unwrap();
            assert_eq!(raw.len(), 1);
            assert_eq!(raw[0].total_tokens, Some(20));
            assert!(matches!(raw[0].status, RecordStatus::Error { .. }));
        }
    }

    #[tokio::test]
    async fn usage_metadata_json_drop_keeps_completed_usage_before_truncated_payload() {
        for partial in [false, true] {
            let (collector, pool) = collector_with_pool().await;
            collector
                .accept(
                    "truncated-image-metadata",
                    RequestType::ImageGeneration,
                    serde_json::json!({"model":"truncated-image-metadata"}),
                )
                .await
                .unwrap();
            let usage = if partial {
                r#"{"input_tokens":12}"#
            } else {
                r#"{"input_tokens":12,"output_tokens":8,"total_tokens":20}"#
            };
            let payload = format!(
                "{{\"usage\":{usage},\"data\":[{{\"b64_json\":\"{}",
                "A".repeat(1024 * 1024 + 1)
            );
            let stream =
                futures::stream::once(async move { Ok::<_, std::io::Error>(Bytes::from(payload)) })
                    .chain(futures::stream::pending());
            let mut response_body = body(&collector, Body::from_stream(stream), false);
            let first =
                futures::future::poll_fn(|cx| Pin::new(&mut response_body).poll_frame(cx)).await;
            assert!(first.unwrap().unwrap().data_ref().unwrap().len() > 1024 * 1024);
            drop(response_body);
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                collector.storage.drain_usage_writes(),
            )
            .await
            .unwrap();

            let raw = collector.storage.load_records().await.unwrap();
            assert_eq!(raw.len(), 1);
            assert_eq!(
                raw[0].input_tokens,
                Some(12),
                "a completed usage object must survive a later payload EOF"
            );
            assert_eq!(raw[0].output_tokens, if partial { None } else { Some(8) });
            assert_eq!(raw[0].total_tokens, if partial { None } else { Some(20) });
            assert!(matches!(raw[0].status, RecordStatus::Error { .. }));
            let daily: (i64, i64, i64, i64) = sqlx::query_as("SELECT request_count, failed_requests, provider_count, missing_count FROM usage_daily")
                .fetch_one(&pool).await.unwrap();
            assert_eq!(daily, if partial { (1, 1, 0, 1) } else { (1, 1, 1, 0) });
            let provenance: (String, String, String) = sqlx::query_as(
                "SELECT input_source, output_source, total_source FROM usage_receipts",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                provenance,
                (
                    "provider".into(),
                    if partial {
                        "missing".into()
                    } else {
                        "provider".into()
                    },
                    if partial {
                        "missing".into()
                    } else {
                        "provider".into()
                    }
                )
            );
        }
    }

    #[tokio::test]
    async fn usage_metadata_body_chunk_boundaries_preserve_nested_fields() {
        let collector = collector().await;
        collector
            .accept(
                "chunked-metadata",
                RequestType::Responses,
                serde_json::json!({"model":"chunked-metadata"}),
            )
            .await
            .unwrap();
        let payload =
            br#"{"response":{"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20}}}"#;
        let chunks: Vec<_> = payload
            .chunks(3)
            .map(|chunk| Ok::<_, std::io::Error>(Bytes::copy_from_slice(chunk)))
            .collect();
        let response_body = body(
            &collector,
            Body::from_stream(futures::stream::iter(chunks)),
            false,
        );
        let bytes = axum::body::to_bytes(response_body, 1024).await.unwrap();
        assert_eq!(bytes.as_ref(), payload);
        collector.storage.drain_usage_writes().await;
        let raw = collector.storage.load_records().await.unwrap();
        assert_eq!(raw[0].total_tokens, Some(20));
        assert!(matches!(raw[0].status, RecordStatus::Success));
    }
}
