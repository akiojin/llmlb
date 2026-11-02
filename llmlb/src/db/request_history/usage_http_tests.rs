//! Real handler coverage for the inference admission boundary (Issue #947).

use super::*;
use crate::common::auth::{ApiKeyPermission, UserRole};
use crate::db::organizations::{self, OrganizationRole};
use crate::types::endpoint::{
    Endpoint, EndpointCapability, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use wiremock::{
    matchers::{method, path_regex},
    Mock, MockServer, ResponseTemplate,
};

fn multipart(fields: &[(&str, &str, bool)]) -> (String, String) {
    let boundary = "usage-fixture-boundary";
    let mut body = String::new();
    for (name, value, file) in fields {
        let filename = if *file {
            "; filename=\"fixture.bin\""
        } else {
            ""
        };
        body.push_str(&format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"{filename}\r\n\r\n{value}\r\n"));
    }
    body.push_str(&format!("--{boundary}--\r\n"));
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn settled_totals(pool: &SqlitePool, expected: i64) -> (i64, i64) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let totals: (i64, i64) = sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(total_tokens), 0) FROM usage_receipts WHERE finalized = 1")
                .fetch_one(pool).await.unwrap();
            if totals.0 == expected { return totals; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("every accepted HTTP request must settle once")
}

#[tokio::test]
async fn usage_http_all_inference_routes_keep_usage_owner_separate_from_issuer() {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let issuer = crate::db::users::create(
        &pool,
        "usage-global@example.com",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let account = crate::db::users::create(
        &pool,
        "usage-member@example.com",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let organization = organizations::create(&pool, "usage organization")
        .await
        .unwrap();
    let organization_id = Uuid::parse_str(&organization.id).unwrap();
    organizations::set_membership(
        &pool,
        account.id,
        Some(organization_id),
        OrganizationRole::Member,
        None,
    )
    .await
    .unwrap();
    let key = crate::db::api_keys::create_attributed(
        &pool,
        "member key",
        issuer.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
        account.id,
        organization_id,
    )
    .await
    .unwrap();
    let key2 = crate::db::api_keys::create_attributed(
        &pool,
        "second member key",
        issuer.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
        account.id,
        organization_id,
    )
    .await
    .unwrap();
    let jwt = crate::auth::jwt::create_jwt(
        &account.id.to_string(),
        UserRole::Viewer,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let global_jwt = crate::auth::jwt::create_jwt(
        &issuer.id.to_string(),
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let upstream = MockServer::start().await;
    let base = format!("/usage-{}", Uuid::new_v4());
    Mock::given(method("POST")).and(path_regex(format!("^{base}/v1/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "usage-fixture", "choices": [{"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":12,"completion_tokens":8,"total_tokens":20}
        }))).mount(&upstream).await;
    let mut endpoint = Endpoint::new(
        "all usage routes".into(),
        format!("{}{base}", upstream.uri()),
        EndpointType::Vllm,
    );
    endpoint.status = EndpointStatus::Online;
    endpoint.capabilities = vec![
        EndpointCapability::ChatCompletion,
        EndpointCapability::AudioTranscription,
        EndpointCapability::AudioSpeech,
        EndpointCapability::ImageGeneration,
    ];
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "usage-model".into(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![
                SupportedAPI::ChatCompletions,
                SupportedAPI::Responses,
                SupportedAPI::Embeddings,
            ],
            canonical_name: None,
        })
        .await
        .unwrap();
    let app = crate::api::create_app(state);
    let json_routes: Vec<(String, Value, String, bool)> = vec![
        (
            "/v1/chat/completions".into(),
            json!({"messages":[{"role":"user","content":"hello"}]}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/chat/completions".into(),
            json!({"messages":[{"role":"user","content":"hello"}]}),
            key2.key,
            false,
        ),
        (
            "/v1/completions".into(),
            json!({"prompt":"hello"}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/embeddings".into(),
            json!({"input":"hello"}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/responses".into(),
            json!({"input":"hello"}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/audio/speech".into(),
            json!({"input":"hello"}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/images/generations".into(),
            json!({"prompt":"hello"}),
            key.key.clone(),
            false,
        ),
        (
            "/v1/messages".into(),
            json!({"max_tokens":16,"messages":[{"role":"user","content":"hello"}]}),
            key.key.clone(),
            true,
        ),
        (
            "/api/dashboard/playground/chat/completions".into(),
            json!({"messages":[{"role":"user","content":"hello"}]}),
            jwt.clone(),
            false,
        ),
        (
            format!("/api/endpoints/{endpoint_id}/chat/completions"),
            json!({"messages":[{"role":"user","content":"hello"}]}),
            jwt,
            false,
        ),
        (
            "/api/dashboard/playground/load-test/chat/completions".into(),
            json!({"messages":[{"role":"user","content":"hello"}]}),
            global_jwt,
            false,
        ),
    ];
    for (path, mut payload, credential, anthropic) in json_routes {
        payload["model"] = json!("usage-model");
        payload["organization_id"] = json!(Uuid::new_v4());
        payload["request_id"] = json!("same-caller-supplied-id");
        let mut request = Request::builder()
            .method("POST")
            .uri(&path)
            .header("content-type", "application/json")
            .extension(ConnectInfo(
                "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
            ));
        request = if anthropic {
            request
                .header("x-api-key", &credential)
                .header("anthropic-version", "2023-06-01")
        } else {
            request.header("authorization", format!("Bearer {credential}"))
        };
        let response = app
            .clone()
            .oneshot(request.body(Body::from(payload.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    for (path, fields) in [
        (
            "/v1/audio/transcriptions",
            vec![
                ("model", "usage-model", false),
                ("file", "fixture audio", true),
            ],
        ),
        (
            "/v1/images/edits",
            vec![
                ("model", "usage-model", false),
                ("image", "fixture image", true),
                ("prompt", "edit", false),
            ],
        ),
        (
            "/v1/images/variations",
            vec![
                ("model", "usage-model", false),
                ("image", "fixture image", true),
            ],
        ),
    ] {
        let (content_type, body) = multipart(&fields);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("authorization", format!("Bearer {}", key.key))
                    .header("content-type", content_type)
                    .extension(ConnectInfo(
                        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                    ))
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    assert_eq!(settled_totals(&pool, 14).await, (14, 280));
    let owner: (i64, i64) = sqlx::query_as("SELECT SUM(request_count), SUM(total_tokens) FROM usage_daily WHERE account_id = ? AND organization_id = ?")
        .bind(account.id.to_string()).bind(organization.id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(
        owner,
        (13, 260),
        "both keys and JWT use the account, not the issuer"
    );
    let issuer_requests: i64 =
        sqlx::query_scalar("SELECT SUM(request_count) FROM usage_daily WHERE account_id = ?")
            .bind(issuer.id.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        issuer_requests, 1,
        "the issuer has only its explicit Load Test request"
    );
    let raw_client_ips: Vec<Option<String>> =
        sqlx::query_scalar("SELECT client_ip FROM request_history")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        raw_client_ips.len(),
        14,
        "each accepted route must retain raw history"
    );
    assert!(
        raw_client_ips
            .iter()
            .all(|ip| ip.as_deref() == Some("127.0.0.1")),
        "every inference route must retain the ConnectInfo client IP: {raw_client_ips:?}"
    );
}

struct StreamFixture {
    app: axum::Router,
    pool: SqlitePool,
    account_id: Uuid,
    issuer_id: Uuid,
    organization_id: Uuid,
    key: String,
    jwt: String,
    upstream_path: String,
    _upstream: MockServer,
}

async fn stream_fixture() -> StreamFixture {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let issuer = crate::db::users::create(
        &pool,
        "stream-global@example.com",
        "hash",
        UserRole::Admin,
        false,
    )
    .await
    .unwrap();
    let account = crate::db::users::create(
        &pool,
        "stream-member@example.com",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    let organization = organizations::create(&pool, "stream original organization")
        .await
        .unwrap();
    let organization_id = Uuid::parse_str(&organization.id).unwrap();
    organizations::set_membership(
        &pool,
        account.id,
        Some(organization_id),
        OrganizationRole::Member,
        None,
    )
    .await
    .unwrap();
    let key = crate::db::api_keys::create_attributed(
        &pool,
        "stream key",
        issuer.id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
        account.id,
        organization_id,
    )
    .await
    .unwrap();
    let jwt = crate::auth::jwt::create_jwt(
        &account.id.to_string(),
        UserRole::Viewer,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let upstream = MockServer::start().await;
    let base = format!("/stream-{}", Uuid::new_v4());
    Mock::given(method("POST")).and(path_regex(format!("^{base}/v1/chat/completions$")))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream").set_body_string(concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":6,\"total_tokens\":10}}\n\n",
            "data: [DONE]\n\n"
        ))).mount(&upstream).await;
    let mut endpoint = Endpoint::new(
        "stream usage fixture".into(),
        format!("{}{base}", upstream.uri()),
        EndpointType::Vllm,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "stream-usage-model".into(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .unwrap();
    StreamFixture {
        app: crate::api::create_app(state),
        pool,
        account_id: account.id,
        issuer_id: issuer.id,
        organization_id,
        key: key.key,
        jwt,
        upstream_path: format!("{base}/v1/chat/completions"),
        _upstream: upstream,
    }
}

async fn request_stream(fixture: &StreamFixture, key: &str) -> axum::response::Response {
    fixture.app.clone().oneshot(Request::builder().method("POST").uri("/v1/chat/completions")
        .header("authorization", format!("Bearer {key}")).header("content-type", "application/json")
        .extension(ConnectInfo("127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap()))
        .body(Body::from(json!({"model":"stream-usage-model","messages":[{"role":"user","content":"hello"}],"stream":true}).to_string())).unwrap())
        .await.unwrap()
}

#[tokio::test]
async fn usage_http_stream_drop_settles_once_without_inventing_reported_zero() {
    let fixture = stream_fixture().await;
    let response = request_stream(&fixture, &fixture.key).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    assert_eq!(settled_totals(&fixture.pool, 1).await, (1, 0));
    let row: (i64, i64, i64, i64) = sqlx::query_as("SELECT SUM(request_count), SUM(successful_requests), SUM(failed_requests), SUM(missing_count) FROM usage_daily")
        .fetch_one(&fixture.pool).await.unwrap();
    assert_eq!(row, (1, 0, 1, 1));
    let tokens: (Option<i64>, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT input_tokens, output_tokens, total_tokens FROM usage_receipts")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(tokens, (None, None, None));
}

#[tokio::test]
async fn usage_http_admission_database_failure_never_forwards() {
    let fixture = stream_fixture().await;
    sqlx::query("CREATE TRIGGER reject_usage_admission BEFORE INSERT ON usage_receipts BEGIN SELECT RAISE(ABORT, 'admission storage unavailable'); END")
        .execute(&fixture.pool).await.unwrap();
    let response = request_stream(&fixture, &fixture.key).await;
    let status = response.status();
    to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert!(
        status.is_server_error(),
        "inference must fail closed when its admission cannot be persisted; got {status}"
    );
    let forwarded = fixture._upstream.received_requests().await.unwrap();
    assert!(
        !forwarded
            .iter()
            .any(|request| request.method.as_str() == "POST"
                && request.url.path() == fixture.upstream_path),
        "a non-durable admission must never reach this upstream fixture"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usage_receipts")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn usage_http_large_image_json_preserves_provider_usage() {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let upstream = MockServer::start().await;
    let base = format!("/large-image-{}", Uuid::new_v4());
    Mock::given(method("POST"))
        .and(path_regex(format!("^{base}/v1/images/generations$")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"b64_json": "A".repeat(2 * 1024 * 1024)}],
            "usage": {"input_tokens":12,"output_tokens":8,"total_tokens":20}
        })))
        .mount(&upstream)
        .await;
    let mut endpoint = Endpoint::new(
        "large image usage".into(),
        format!("{}{base}", upstream.uri()),
        EndpointType::Vllm,
    );
    endpoint.status = EndpointStatus::Online;
    endpoint.capabilities = vec![EndpointCapability::ImageGeneration];
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .unwrap();
    let response = crate::api::create_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("authorization", "Bearer sk_debug")
                .header("content-type", "application/json")
                .extension(ConnectInfo(
                    "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .body(Body::from(
                    json!({"model":"large-image-model","prompt":"draw"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    assert!(bytes.len() > 1024 * 1024);
    assert_eq!(
        settled_totals(&pool, 1).await,
        (1, 20),
        "large non-usage values must not erase valid provider counts"
    );
    let measured: i64 = sqlx::query_scalar("SELECT SUM(provider_count) FROM usage_daily")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(measured, 1);
}

#[tokio::test]
async fn usage_http_stream_transfer_keeps_original_snapshot_and_rejects_old_key() {
    let fixture = stream_fixture().await;
    let response = request_stream(&fixture, &fixture.key).await;
    assert_eq!(response.status(), StatusCode::OK);
    let organization = organizations::create(&fixture.pool, "stream destination organization")
        .await
        .unwrap();
    let destination = Uuid::parse_str(&organization.id).unwrap();
    organizations::set_membership(
        &fixture.pool,
        fixture.account_id,
        Some(destination),
        OrganizationRole::Member,
        None,
    )
    .await
    .unwrap();
    to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(settled_totals(&fixture.pool, 1).await, (1, 10));
    let first_scope: (String, String) =
        sqlx::query_as("SELECT organization_id, account_id FROM usage_receipts")
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(
        first_scope,
        (
            fixture.organization_id.to_string(),
            fixture.account_id.to_string()
        )
    );
    let denied = request_stream(&fixture, &fixture.key).await;
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let new_key = crate::db::api_keys::create_attributed(
        &fixture.pool,
        "destination key",
        fixture.issuer_id,
        None,
        vec![ApiKeyPermission::OpenaiInference],
        fixture.account_id,
        destination,
    )
    .await
    .unwrap();
    let response = request_stream(&fixture, &new_key.key).await;
    assert_eq!(response.status(), StatusCode::OK);
    to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(settled_totals(&fixture.pool, 2).await, (2, 20));
    let current_history = RequestHistoryStorage::new(fixture.pool.clone())
        .scoped(crate::auth::scope::UsageScope::Account {
            organization_id: destination,
            account_id: fixture.account_id,
        })
        .load_records()
        .await
        .unwrap();
    assert_eq!(
        current_history.len(),
        1,
        "the new organization cannot read the old stream history"
    );
    let usage_response = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/dashboard/usage")
                .header("authorization", format!("Bearer {}", fixture.jwt))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(usage_response.status(), StatusCode::OK);
    let usage: Value = serde_json::from_slice(
        &to_bytes(usage_response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        usage["totals"]["request_count"], 1,
        "old JWT resolves the current membership rather than exposing old organization totals"
    );
    organizations::archive(&fixture.pool, fixture.organization_id)
        .await
        .unwrap();
    let original_tokens: i64 =
        sqlx::query_scalar("SELECT SUM(total_tokens) FROM usage_daily WHERE organization_id = ?")
            .bind(fixture.organization_id.to_string())
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(original_tokens, 10);
}
