//! One inference receipt and terminal observation per accepted HTTP request.

use crate::auth::{middleware::ApiKeyAuthContext, scope::Scope};
use crate::common::{
    auth::Claims,
    error::LbError,
    protocol::{RecordStatus, RequestResponseRecord, RequestType},
    usage::{UsageAttribution, UsageTokens},
};
use crate::db::request_history::RequestHistoryStorage;
use crate::AppState;
use axum::{
    body::{Body, Bytes},
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use http_body::{Body as HttpBody, Frame, SizeHint};
use metadata::MetadataScanner;
use serde_json::Value;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll},
    time::Instant,
};
use uuid::Uuid;

#[cfg(test)]
mod adapter_tests;
mod metadata;
#[cfg(test)]
mod tests;

tokio::task_local! {
    static CURRENT_CAPTURE: Arc<Capture>;
}

#[derive(Default)]
struct CapturedState {
    record: Option<RequestResponseRecord>,
    admitted: bool,
    has_handler_record: bool,
    finalized: bool,
    direct_upstream: bool,
    upstream_provider: Option<UsageTokens>,
    upstream_estimate: Option<UsageTokens>,
    semantic_failed: bool,
}

/// Shared with a transforming stream, whose body is polled outside task-local
/// handler scope. Only original upstream bytes are provider authority.
#[derive(Clone)]
pub(crate) struct UpstreamObserver {
    capture: Arc<Capture>,
    scanner: Arc<Mutex<MetadataScanner>>,
}

impl UpstreamObserver {
    pub(crate) fn push(&self, bytes: &[u8]) {
        let mut scanner = self
            .scanner
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        scanner.push(bytes);
        let mut state = self.capture.state();
        state.upstream_provider = scanner.observation();
        state.semantic_failed |= scanner.failed();
    }

    /// Existing accumulator fallback is explicitly estimated, never a wire
    /// provider value. The terminal observer uses it only after normal EOF.
    pub(crate) fn estimate(&self, input: Option<u64>, output: Option<u64>, total: Option<u64>) {
        self.capture.state().upstream_estimate = Some(UsageTokens::estimated(input, output, total));
    }

    /// A structured upstream failure can arrive inside a successful HTTP SSE
    /// response; its status line must not turn the inference into a success.
    pub(crate) fn failed(&self) {
        self.capture.state().semantic_failed = true;
    }
}

/// Mark a response adapter as observing the original provider representation.
pub(crate) fn upstream_observer(content_type: &str) -> Option<UpstreamObserver> {
    CURRENT_CAPTURE
        .try_with(|capture| {
            capture.state().direct_upstream = true;
            UpstreamObserver {
                capture: capture.clone(),
                scanner: Arc::new(Mutex::new(MetadataScanner::new(content_type))),
            }
        })
        .ok()
}

/// Observe a buffered upstream body before conversion. The scanner remains
/// bounded when a JSON body contains giant strings; no body copy is retained.
pub(crate) fn observe_upstream_body(content_type: &str, bytes: &[u8]) {
    if let Some(observer) = upstream_observer(content_type) {
        observer.push(bytes);
    }
}

struct Capture {
    attribution: UsageAttribution,
    client_ip: Option<IpAddr>,
    started: Instant,
    storage: Arc<RequestHistoryStorage>,
    admission: tokio::sync::Mutex<()>,
    state: Mutex<CapturedState>,
}

impl Capture {
    fn state(&self) -> MutexGuard<'_, CapturedState> {
        // A handler panic must not prevent an already durable receipt from being
        // finalized on cancellation. The immutable attribution stays separate.
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    async fn accept(
        &self,
        model: &str,
        request_type: RequestType,
        body: Value,
    ) -> Result<(), LbError> {
        let _admission = self.admission.lock().await;
        if self.state().admitted {
            return Ok(());
        }
        let record = RequestResponseRecord {
            id: self.attribution.request_id,
            timestamp: self.attribution.received_at,
            request_type,
            model: model.to_string(),
            endpoint_id: Uuid::nil(),
            endpoint_name: "N/A".to_string(),
            endpoint_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            client_ip: self.client_ip,
            request_body: body,
            response_body: None,
            duration_ms: 0,
            status: RecordStatus::Error {
                message: "Accepted request has not completed".to_string(),
            },
            completed_at: self.attribution.received_at,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            api_key_id: self.attribution.api_key_id,
        };
        // Keep cancellation metadata before awaiting storage: SQLite may commit
        // while this parent future is never polled again. This is not evidence
        // of acceptance; the Drop path must query the existing receipt first.
        let record = self.state().record.get_or_insert(record).clone();
        self.storage
            .accept_usage(&self.attribution, &record)
            .await?;
        self.state().admitted = true;
        Ok(())
    }

    fn merge_record(&self, incoming: &RequestResponseRecord) -> bool {
        let mut state = self.state();
        if state.finalized {
            return true;
        }
        if !state.admitted {
            return false;
        }
        let Some(record) = state.record.as_mut() else {
            return false;
        };
        if !incoming.endpoint_id.is_nil() {
            record.endpoint_id = incoming.endpoint_id;
            record.endpoint_name.clone_from(&incoming.endpoint_name);
            record.endpoint_ip = incoming.endpoint_ip;
        }
        record.response_body.clone_from(&incoming.response_body);
        record.input_tokens = incoming.input_tokens;
        record.output_tokens = incoming.output_tokens;
        record.total_tokens = incoming.total_tokens;
        record.status.clone_from(&incoming.status);
        state.has_handler_record = true;
        // Caller/client UUID, account, org, time, and key/IP are never authority.
        true
    }

    fn finish(
        &self,
        observed: Option<UsageTokens>,
        status: StatusCode,
        complete: bool,
        textual: bool,
    ) {
        let mut state = self.state();
        if state.finalized {
            return;
        }
        let Some(mut record) = state.record.clone() else {
            return;
        };
        let handler_error = if state.has_handler_record {
            match &record.status {
                RecordStatus::Error { message } => Some(message.clone()),
                RecordStatus::Success => None,
            }
        } else {
            None
        };
        let observed = if state.direct_upstream {
            state.upstream_provider.clone()
        } else {
            observed
        };
        let estimate = state.upstream_estimate.clone();
        let admitted = state.admitted;
        let semantic_failed = state.semantic_failed;
        state.finalized = true;
        drop(state);
        let tokens = observed
            .or_else(|| {
                if complete && textual && !semantic_failed {
                    estimate.or_else(|| {
                        (record.input_tokens.is_some()
                            || record.output_tokens.is_some()
                            || record.total_tokens.is_some())
                        .then(|| {
                            UsageTokens::estimated(
                                record.input_tokens.map(u64::from),
                                record.output_tokens.map(u64::from),
                                record.total_tokens.map(u64::from),
                            )
                        })
                    })
                } else {
                    None
                }
            })
            .unwrap_or_else(UsageTokens::missing);
        record.completed_at = Utc::now();
        record.duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        record.status = if complete && status.is_success() && !semantic_failed {
            RecordStatus::Success
        } else {
            RecordStatus::Error {
                message: handler_error.unwrap_or_else(|| {
                    if semantic_failed {
                        "Upstream stream reported an error".to_string()
                    } else if complete {
                        format!("HTTP {}", status.as_u16())
                    } else {
                        "Response body did not complete".to_string()
                    }
                }),
            }
        };
        // Existing raw DTOs are u32; never wrap a larger provider measurement.
        // The durable usage rollup still retains its full supported i64 value.
        record.input_tokens = tokens
            .input_tokens
            .and_then(|value| u32::try_from(value).ok());
        record.output_tokens = tokens
            .output_tokens
            .and_then(|value| u32::try_from(value).ok());
        record.total_tokens = tokens
            .total_tokens
            .and_then(|value| u32::try_from(value).ok());
        if admitted {
            self.storage
                .enqueue_usage_final(self.attribution.clone(), record, tokens);
        } else {
            self.storage
                .enqueue_usage_cancellation(self.attribution.clone(), record);
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Includes cancellation while the handler is awaiting an upstream and
        // no response Body has been returned to the middleware yet.
        self.finish(None, StatusCode::INTERNAL_SERVER_ERROR, false, false);
    }
}

/// Await durable acceptance after input validation and before the first forward.
///
/// Every inference Router must install the capture middleware after auth. An
/// omitted layer is an admission failure, including in private handler tests.
pub async fn accept_current(
    _state: &AppState,
    model: &str,
    request_type: RequestType,
    body: Value,
) -> Result<(), LbError> {
    let capture = CURRENT_CAPTURE.try_with(Arc::clone).map_err(|_| {
        tracing::error!(
            model,
            ?request_type,
            "Inference usage capture middleware is missing; refusing admission"
        );
        LbError::Internal(
            "Inference usage capture is missing; request was not accepted".to_string(),
        )
    })?;
    capture.accept(model, request_type, body).await
}

/// Absorb a legacy capture helper's target/response metadata into this receipt.
/// Returns true when the caller must suppress its separate raw-history writer.
pub fn capture_record(record: &RequestResponseRecord) -> bool {
    CURRENT_CAPTURE
        .try_with(|capture| capture.merge_record(record))
        .unwrap_or(false)
}

async fn attribution(
    state: &AppState,
    key: Option<ApiKeyAuthContext>,
    claims: Option<Claims>,
    scope: Option<Scope>,
) -> Result<UsageAttribution, crate::api::error::HandlerError> {
    let (account_id, organization_id, api_key_id, auth_source) = if let Some(key) = key {
        // created_by remains issuer audit, including for legacy keys.
        (
            key.usage_account_id,
            key.organization_id,
            Some(key.id),
            "api_key",
        )
    } else {
        let unauthorized =
            || crate::api::error::HandlerError::from(StatusCode::UNAUTHORIZED.into_response());
        let claims = claims.ok_or_else(unauthorized)?;
        let account_id = Uuid::parse_str(&claims.sub).map_err(|_| unauthorized())?;
        let scope = scope.ok_or_else(unauthorized)?;
        scope.require_assigned()?;
        let organization_id = match &scope {
            Scope::Organization(id) => Some(*id),
            Scope::SelfAccount {
                organization_id,
                account_id: scoped_account,
            } if *scoped_account == account_id => Some(*organization_id),
            Scope::Global => {
                crate::db::organizations::membership(&state.db_pool, account_id)
                    .await
                    .map_err(crate::api::error::AppError)?
                    .ok_or_else(unauthorized)?
                    .organization_id
            }
            _ => return Err(StatusCode::FORBIDDEN.into_response().into()),
        };
        (
            organization_id.map(|_| account_id),
            organization_id,
            None,
            "jwt",
        )
    };
    Ok(UsageAttribution {
        request_id: Uuid::new_v4(),
        account_id,
        organization_id,
        received_at: Utc::now(),
        api_key_id,
        auth_source: auth_source.to_string(),
    })
}

/// Capture server authority after authentication, then observe the response body
/// without altering any response payload, framing, or headers.
pub async fn usage_capture_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let key = request.extensions().get::<ApiKeyAuthContext>().cloned();
    let claims = request.extensions().get::<Claims>().cloned();
    let scope = request.extensions().get::<Scope>().cloned();
    let attribution = match attribution(&state, key, claims, scope).await {
        Ok(attribution) => attribution,
        Err(error) => return error.into_response(),
    };
    let client_ip =
        crate::api::openai::extract_client_ip_from_headers(request.headers()).or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ConnectInfo(address)| crate::common::ip::normalize_socket_ip(address))
        });
    let capture = Arc::new(Capture {
        attribution,
        client_ip,
        started: Instant::now(),
        storage: state.balancer.request_history.clone(),
        admission: tokio::sync::Mutex::new(()),
        state: Mutex::new(CapturedState::default()),
    });
    let response = CURRENT_CAPTURE
        .scope(capture.clone(), next.run(request))
        .await;
    observe_response(response, capture)
}

fn observe_response(response: Response, capture: Arc<Capture>) -> Response {
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let scanner = MetadataScanner::new(content_type);
    response.map(|body| {
        let mut observed = ObservedBody {
            body: Box::pin(body),
            capture,
            scanner,
            status,
            finished: false,
        };
        if observed.body.is_end_stream() {
            observed.finish(true);
        }
        Body::new(observed)
    })
}

/// Explicit post-auth fixture for private forwarder unit tests. This uses the
/// production admission, storage transaction, and terminal Body observer; an
/// absent capture never turns into a special successful test-only admission.
#[cfg(test)]
pub(crate) async fn with_test_capture<F, E>(
    state: &AppState,
    client_ip: Option<IpAddr>,
    api_key_id: Option<Uuid>,
    future: F,
) -> Result<Response, E>
where
    F: std::future::Future<Output = Result<Response, E>>,
{
    let capture = Arc::new(Capture {
        attribution: UsageAttribution {
            request_id: Uuid::new_v4(),
            account_id: None,
            organization_id: None,
            received_at: Utc::now(),
            api_key_id,
            auth_source: "api_key".to_string(),
        },
        client_ip,
        started: Instant::now(),
        storage: state.balancer.request_history.clone(),
        admission: tokio::sync::Mutex::new(()),
        state: Mutex::new(CapturedState::default()),
    });
    match CURRENT_CAPTURE.scope(capture.clone(), future).await {
        Ok(response) => Ok(observe_response(response, capture)),
        Err(error) => {
            capture.finish(None, StatusCode::INTERNAL_SERVER_ERROR, false, false);
            state
                .balancer
                .request_history
                .drain_usage_writers()
                .await
                .expect("unit fixture terminal usage persistence");
            Err(error)
        }
    }
}

struct ObservedBody {
    body: Pin<Box<Body>>,
    capture: Arc<Capture>,
    scanner: MetadataScanner,
    status: StatusCode,
    finished: bool,
}

impl ObservedBody {
    fn finish(&mut self, complete: bool) {
        if !self.finished {
            self.finished = true;
            if self.scanner.failed() {
                self.capture.state().semantic_failed = true;
            }
            self.capture.finish(
                self.scanner.observation(),
                self.status,
                complete,
                self.scanner.textual(),
            );
        }
    }
}

impl HttpBody for ObservedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let result = self.body.as_mut().poll_frame(context);
        match &result {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(bytes) = frame.data_ref() {
                    self.scanner.push(bytes);
                }
                if self.body.is_end_stream() {
                    self.finish(true);
                }
            }
            Poll::Ready(Some(Err(_))) => self.finish(false),
            Poll::Ready(None) => self.finish(true),
            Poll::Pending => {}
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

impl Drop for ObservedBody {
    fn drop(&mut self) {
        self.finish(false);
    }
}
