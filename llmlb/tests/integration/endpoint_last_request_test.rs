//! Issue #944: each public inference entry records assignment before response completion.
use crate::support::{
    http::spawn_lb,
    lb::{build_test_router, create_test_db_pool},
};
use axum::{extract::State, routing::post, Router};
use llmlb::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Clone, Default)]
struct PendingResponse {
    started: Arc<Notify>,
    release: Arc<Notify>,
}

async fn pending_stream(
    State(pending): State<PendingResponse>,
) -> impl axum::response::IntoResponse {
    pending.started.notify_one();
    pending.release.notified().await;
    ([ ("content-type", "text/event-stream") ],
     "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n")
}

async fn assert_start_before_response_paths(route: &str, upstream_route: &str, body: Value) {
    let pending = PendingResponse::default();
    let upstream = spawn_lb(
        Router::new()
            .route(upstream_route, post(pending_stream))
            .with_state(pending.clone()),
    )
    .await;
    let pool = create_test_db_pool().await;
    let mut endpoint = Endpoint::new(
        "start-contract".into(),
        format!("http://{}", upstream.addr()),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    let id = endpoint.id;
    llmlb::db::endpoints::create_endpoint(&pool, &endpoint)
        .await
        .unwrap();
    let model = EndpointModel {
        endpoint_id: id,
        model_id: "start-model".into(),
        capabilities: None,
        max_tokens: None,
        last_checked: None,
        supported_apis: vec![SupportedAPI::ChatCompletions, SupportedAPI::Responses],
        canonical_name: None,
    };
    llmlb::db::endpoints::add_endpoint_model(&pool, &model)
        .await
        .unwrap();
    let user = llmlb::db::users::create(
        &pool,
        "last-request-admin",
        "unused",
        llmlb::common::auth::UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let jwt = llmlb::auth::jwt::create_jwt(
        &user.id.to_string(),
        llmlb::common::auth::UserRole::Admin,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .unwrap();
    let server = spawn_lb(build_test_router(pool.clone()).await).await;
    let client = reqwest::Client::new();
    let request = tokio::spawn({
        let client = client.clone();
        let url = format!("http://{}{route}", server.addr());
        async move {
            client
                .post(url)
                .header("x-api-key", "sk_debug")
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        pending.started.notified(),
    )
    .await
    .expect("inference reached upstream");
    assert!(!request.is_finished(), "stream response has not completed");
    let recorded = llmlb::db::endpoints::get_endpoint(&pool, id)
        .await
        .unwrap()
        .unwrap();
    let timestamp = recorded
        .last_request_at
        .expect("durable start before upstream response");
    let dashboard: Vec<Value> = client
        .get(format!("http://{}/api/dashboard/endpoints", server.addr()))
        .bearer_auth(&jwt)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dashboard[0]["last_request_at"], json!(timestamp));
    pending.release.notify_one();
    let response = request.await.unwrap();
    assert!(
        response.status().is_success(),
        "route {route}: {}",
        response.status()
    );
    let _ = response.bytes().await.unwrap();
    assert_eq!(
        llmlb::db::endpoints::get_endpoint(&pool, id)
            .await
            .unwrap()
            .unwrap()
            .last_request_at,
        Some(timestamp)
    );
}

#[tokio::test]
async fn last_request_openai_stream_is_recorded_at_assignment() {
    assert_start_before_response_paths(
        "/v1/chat/completions",
        "/v1/chat/completions",
        json!({"model":"start-model","messages":[{"role":"user","content":"ping"}],"stream":true}),
    )
    .await;
}

#[tokio::test]
async fn last_request_anthropic_stream_is_recorded_at_assignment() {
    // The Anthropic adapter forwards to an OpenAI-compatible chat upstream.
    let body = json!({"model":"start-model","messages":[{"role":"user","content":"ping"}],"max_tokens":8,"stream":true});
    assert_start_before_response_paths("/v1/messages", "/v1/chat/completions", body).await;
}

#[tokio::test]
async fn last_request_responses_stream_is_recorded_at_assignment() {
    assert_start_before_response_paths(
        "/v1/responses",
        "/v1/responses",
        json!({"model":"start-model","input":"ping","stream":true}),
    )
    .await;
}
