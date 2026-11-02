//! Real-router checks: an adapter's synthetic wire usage is never provider data.
use crate::{
    common::auth::{ApiKeyPermission, UserRole},
    types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI},
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

async fn observation(usage: Option<Value>, stream: bool) -> (String, i64, i64, i64, i64) {
    observation_with_error(usage, stream, false).await
}

async fn observation_with_error(
    usage: Option<Value>,
    stream: bool,
    error: bool,
) -> (String, i64, i64, i64, i64) {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let issuer = crate::db::users::create(&state.db_pool, "issuer", "hash", UserRole::Admin, false)
        .await
        .unwrap();
    let key = crate::db::api_keys::create(
        &state.db_pool,
        "legacy-inference",
        issuer.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
    )
    .await
    .unwrap();
    let upstream = MockServer::start().await;
    let mut payload = if stream {
        json!({"id":"chatcmpl-upstream","choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]})
    } else {
        json!({"id":"chatcmpl-upstream","choices":[{"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}]})
    };
    if let Some(usage) = usage {
        payload["usage"] = usage;
    }
    if error {
        payload = json!({"error":{"message":"upstream stream failed"}});
    }
    let response = if stream {
        ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(format!("data: {payload}\n\ndata: [DONE]\n\n"))
    } else {
        ResponseTemplate::new(200).set_body_json(payload)
    };
    Mock::given(method("POST"))
        .respond_with(response)
        .mount(&upstream)
        .await;
    let mut endpoint = Endpoint::new(
        "adapter upstream".to_string(),
        upstream.uri(),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    state
        .balancer
        .endpoint_registry
        .add(endpoint.clone())
        .await
        .unwrap();
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id: endpoint.id,
            model_id: "fixture-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .unwrap();
    let app = crate::api::create_app(state.clone());
    let mut request = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("x-api-key", &key.key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"model":"fixture-model","max_tokens":32,
            "messages":[{"role":"user","content":"hello"}],"stream":stream}))
            .unwrap(),
        ))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:9".parse::<std::net::SocketAddr>().unwrap(),
    ));
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    if stream {
        assert!(String::from_utf8(bytes.to_vec())
            .unwrap()
            .contains(if error {
                "event: error"
            } else {
                "event: message_stop"
            }));
    } else {
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["type"], "message");
        assert_eq!(wire["content"][0]["text"], "hello");
    }
    state
        .balancer
        .request_history
        .drain_usage_writers()
        .await
        .unwrap();
    if error {
        let outcome: Option<String> = sqlx::query_scalar("SELECT outcome FROM usage_receipts")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            Some("error".to_string()),
            "HTTP200 SSE error is a failed inference outcome"
        );
    }
    sqlx::query_as(
        "SELECT r.token_source,d.total_tokens,d.input_missing_count,d.output_missing_count,d.total_missing_count
         FROM usage_receipts r JOIN usage_daily d ON d.date=r.date
         AND d.organization_id=COALESCE(r.organization_id,'') AND d.account_id=COALESCE(r.account_id,'')",
    ).fetch_one(&state.db_pool).await.unwrap()
}

async fn assert_provenance(stream: bool) {
    assert_eq!(
        observation(
            Some(json!({"prompt_tokens":3,"completion_tokens":4,"total_tokens":7})),
            stream
        )
        .await,
        ("provider".to_string(), 7, 0, 0, 0),
        "the provider total must survive conversion even though Anthropic wire usage omits it",
    );
    assert_eq!(
        observation(Some(json!({"prompt_tokens":3})), stream).await,
        ("provider".to_string(), 0, 0, 1, 1),
        "synthetic wire output zero is still missing upstream coverage",
    );
    let no_usage = observation(None, stream).await;
    assert_eq!(
        no_usage.0, "estimated",
        "adapter estimates are never provider measurements"
    );
    assert!(no_usage.1 > 0);
    assert_eq!((no_usage.2, no_usage.3, no_usage.4), (0, 0, 0));
    assert_eq!(
        observation(Some(json!({})), stream).await,
        ("missing".to_string(), 0, 1, 1, 1)
    );
    assert_eq!(
        observation(
            Some(json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0})),
            stream
        )
        .await,
        ("provider".to_string(), 0, 0, 0, 0)
    );
}

#[tokio::test]
async fn anthropic_nonstream_retains_provider_total_and_partial_or_estimated_provenance() {
    assert_provenance(false).await;
}

#[tokio::test]
async fn anthropic_stream_retains_provider_total_and_partial_or_estimated_provenance() {
    assert_provenance(true).await;
}

#[tokio::test]
async fn anthropic_sse_error_event_is_an_error_outcome_without_synthetic_measured_zero() {
    let snapshot = observation_with_error(None, true, true).await;
    assert_eq!(snapshot, ("missing".to_string(), 0, 1, 1, 1));
}
