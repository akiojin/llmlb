//! Inference request gate for self-update.
//!
//! - Tracks in-flight `/v1/*` inference requests (including streaming).
//! - When rejecting is enabled, new requests are rejected with 503.

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use http_body::{Body as HttpBody, Frame, SizeHint};
use serde_json::json;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::sync::Notify;

/// Gate shared across the server.
#[derive(Clone, Debug, Default)]
pub struct InferenceGate {
    inner: Arc<InferenceGateInner>,
}

#[derive(Debug, Default)]
struct InferenceGateInner {
    rejecting: AtomicBool,
    in_flight: AtomicUsize,
    idle_notify: Notify,
    abort_generation: AtomicU64,
    abort_notify: Notify,
}

impl InferenceGate {
    /// Return current in-flight request count.
    pub fn in_flight(&self) -> usize {
        self.inner.in_flight.load(Ordering::Relaxed)
    }

    /// Returns true when the gate is rejecting new requests.
    pub fn is_rejecting(&self) -> bool {
        self.inner.rejecting.load(Ordering::Relaxed)
    }

    /// Begin rejecting new inference requests.
    pub fn start_rejecting(&self) {
        self.inner.rejecting.store(true, Ordering::SeqCst);
    }

    /// Stop rejecting new inference requests.
    pub fn stop_rejecting(&self) {
        self.inner.rejecting.store(false, Ordering::SeqCst);
    }

    /// Wait until all in-flight requests complete.
    pub async fn wait_for_idle(&self) {
        loop {
            if self.in_flight() == 0 {
                return;
            }
            self.inner.idle_notify.notified().await;
        }
    }

    /// Increment force-abort generation and notify all in-flight requests.
    pub fn abort_in_flight(&self) {
        self.inner.abort_generation.fetch_add(1, Ordering::SeqCst);
        self.inner.abort_notify.notify_waiters();
    }

    fn begin(&self) -> InFlightGuard {
        self.inner.in_flight.fetch_add(1, Ordering::SeqCst);
        InFlightGuard { gate: self.clone() }
    }

    fn finish(&self) {
        let prev = self.inner.in_flight.fetch_sub(1, Ordering::SeqCst);
        let now = prev.saturating_sub(1);
        if now == 0 {
            self.inner.idle_notify.notify_waiters();
        }
    }

    fn abort_generation(&self) -> u64 {
        self.inner.abort_generation.load(Ordering::SeqCst)
    }

    fn is_force_aborted_since(&self, generation: u64) -> bool {
        self.abort_generation() != generation
    }

    async fn wait_for_force_abort_since(&self, generation: u64) {
        loop {
            if self.is_force_aborted_since(generation) {
                return;
            }
            self.inner.abort_notify.notified().await;
        }
    }
}

#[derive(Debug)]
struct InFlightGuard {
    gate: InferenceGate,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.gate.finish();
    }
}

#[derive(Debug)]
struct InFlightBody {
    inner: Body,
    gate: InferenceGate,
    abort_generation: u64,
    _guard: InFlightGuard,
}

impl HttpBody for InFlightBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.gate.is_force_aborted_since(self.abort_generation) {
            return Poll::Ready(None);
        }
        Pin::new(&mut self.inner).poll_frame(cx)
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
}

fn service_unavailable_updating_response() -> Response {
    let mut response = (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": {
                "message": "Server is updating. Please retry.",
                "type": "service_unavailable",
                "code": StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            }
        })),
    )
        .into_response();

    if let Ok(value) = HeaderValue::from_str("30") {
        response
            .headers_mut()
            .insert(HeaderName::from_static("retry-after"), value);
    }

    response
}

/// Middleware that counts in-flight inference requests and rejects new ones when draining.
pub async fn inference_gate_middleware(
    State(gate): State<InferenceGate>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if gate.is_rejecting() {
        return service_unavailable_updating_response();
    }

    let guard = gate.begin();
    let abort_generation = gate.abort_generation();
    let abort_wait = gate.wait_for_force_abort_since(abort_generation);
    tokio::pin!(abort_wait);

    let res = tokio::select! {
        res = next.run(req) => res,
        _ = &mut abort_wait => return service_unavailable_updating_response(),
    };

    let (parts, body) = res.into_parts();
    let body = Body::new(InFlightBody {
        inner: body,
        gate: gate.clone(),
        abort_generation,
        _guard: guard,
    });
    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
        middleware,
        routing::post,
        Router,
    };
    use futures::stream;
    use std::time::Duration;
    use tower::ServiceExt;

    #[tokio::test]
    async fn wait_for_idle_waits_until_in_flight_zero() {
        let gate = InferenceGate::default();
        let guard = gate.begin();

        let gate2 = gate.clone();
        let drop_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(guard);
            gate2.in_flight()
        });

        // Should not be idle yet.
        assert!(
            tokio::time::timeout(Duration::from_millis(10), gate.wait_for_idle())
                .await
                .is_err()
        );

        let _ = drop_task.await.unwrap();
        gate.wait_for_idle().await;
        assert_eq!(gate.in_flight(), 0);
    }

    #[tokio::test]
    async fn middleware_holds_in_flight_until_response_body_is_dropped() {
        let gate = InferenceGate::default();

        let app = Router::new()
            .route("/v1/test", post(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                gate.clone(),
                inference_gate_middleware,
            ));

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(gate.in_flight(), 1);
        drop(res);
        tokio::task::yield_now().await;
        assert_eq!(gate.in_flight(), 0);
    }

    #[tokio::test]
    async fn middleware_force_abort_cancels_in_flight_handler() {
        let gate = InferenceGate::default();

        let app = Router::new()
            .route(
                "/v1/test",
                post(|| async {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    "ok"
                }),
            )
            .layer(middleware::from_fn_with_state(
                gate.clone(),
                inference_gate_middleware,
            ));

        let request_task = tokio::spawn(async move {
            app.oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response")
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        gate.start_rejecting();
        gate.abort_in_flight();

        let res = tokio::time::timeout(Duration::from_secs(1), request_task)
            .await
            .expect("request task should complete quickly")
            .expect("task join should succeed");

        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(gate.in_flight(), 0);
    }

    #[tokio::test]
    async fn force_abort_finishes_streaming_response_body() {
        let gate = InferenceGate::default();

        let app = Router::new()
            .route(
                "/v1/test",
                post(|| async {
                    let never_stream = stream::pending::<Result<Bytes, std::convert::Infallible>>();
                    Body::from_stream(never_stream)
                }),
            )
            .layer(middleware::from_fn_with_state(
                gate.clone(),
                inference_gate_middleware,
            ));

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/test")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(gate.in_flight(), 1);
        gate.start_rejecting();
        gate.abort_in_flight();

        let bytes = tokio::time::timeout(
            Duration::from_millis(100),
            to_bytes(res.into_body(), usize::MAX),
        )
        .await
        .expect("force-aborted body should finish quickly");
        let bytes = bytes.expect("force-aborted body should be readable");
        assert!(
            bytes.is_empty(),
            "force-aborted streaming response should finish without body payload"
        );
        assert_eq!(gate.in_flight(), 0);
    }
}
