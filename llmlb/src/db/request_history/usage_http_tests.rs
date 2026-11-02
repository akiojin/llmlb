//! Every production inference entry must retain one server-selected receipt.
use crate::{
    common::auth::{ApiKeyPermission, UserRole},
    db::organizations::{self, OrganizationRole},
    types::endpoint::{
        Endpoint, EndpointCapability, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    },
};
use axum::{
    body::{to_bytes, Body},
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::Row;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

fn request(
    path: &str,
    key: &str,
    content_type: &str,
    body: Vec<u8>,
    anthropic: bool,
) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", content_type)
        .header("x-forwarded-for", "192.0.2.37")
        .header("x-organization-id", Uuid::new_v4().to_string())
        .header("x-request-id", "client-controlled-not-authority");
    request = if anthropic {
        request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.header("authorization", format!("Bearer {key}"))
    };
    let mut request = request.body(Body::from(body)).unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
    ));
    request
}

fn multipart(field: &str, prompt: bool) -> Vec<u8> {
    let mut body="--receipt-boundary\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nfixture-model\r\n".to_string();
    if prompt {
        body.push_str("--receipt-boundary\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nchange image\r\n");
    }
    body.push_str(&format!("--receipt-boundary\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"fixture.png\"\r\nContent-Type: image/png\r\n\r\nfixture-bytes\r\n--receipt-boundary--\r\n"));
    body.into_bytes()
}

#[tokio::test]
async fn all_inference_routes_capture_once_with_issuer_independent_snapshot_and_ip() {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let pool = state.db_pool.clone();
    let storage = state.balancer.request_history.clone();
    let issuer = crate::db::users::create(&pool, "route-issuer", "hash", UserRole::Admin, false)
        .await
        .unwrap();
    let consumer =
        crate::db::users::create(&pool, "route-consumer", "hash", UserRole::Admin, false)
            .await
            .unwrap();
    let org = organizations::create(&pool, "Route Alpha").await.unwrap();
    organizations::set_membership(&pool, consumer.id, Some(org.id), OrganizationRole::Member)
        .await
        .unwrap();
    let mut keys = Vec::new();
    for name in ["first", "second"] {
        keys.push(
            crate::db::api_keys::create_attributed(
                &pool,
                name,
                issuer.id,
                Some(consumer.id),
                Some(org.id),
                None,
                vec![
                    ApiKeyPermission::OpenaiInference,
                    ApiKeyPermission::OpenaiModelsRead,
                ],
            )
            .await
            .unwrap(),
        );
    }
    let jwt = crate::auth::jwt::create_jwt(
        &consumer.id.to_string(),
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let upstream = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"fixture-response","model":"fixture-model","choices":[{"message":{"role":"assistant","content":"hello"},"text":"hello","finish_reason":"stop"}],
        "data":[{"embedding":[0.1],"url":"https://example.invalid/generated.png"}],"text":"hello","output":[],
        "usage":{"prompt_tokens":3,"completion_tokens":4,"input_tokens":3,"output_tokens":4,"total_tokens":7}
    }))).mount(&upstream).await;
    let mut endpoint = Endpoint::new("receipt target".into(), upstream.uri(), EndpointType::Xllm);
    endpoint.status = EndpointStatus::Online;
    endpoint.capabilities = vec![
        EndpointCapability::ChatCompletion,
        EndpointCapability::Embeddings,
        EndpointCapability::AudioTranscription,
        EndpointCapability::AudioSpeech,
        EndpointCapability::ImageGeneration,
    ];
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
            model_id: "fixture-model".into(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![
                SupportedAPI::ChatCompletions,
                SupportedAPI::Responses,
                SupportedAPI::Embeddings,
                SupportedAPI::ImageGeneration,
            ],
            canonical_name: None,
        })
        .await
        .unwrap();
    let app = crate::api::create_app(state.clone());
    let mut calls: Vec<(String, Value, String, bool)> = vec![
        (
            "/v1/chat/completions".into(),
            json!({"model":"fixture-model","messages":[{"role":"user","content":"hello"}]}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/completions".into(),
            json!({"model":"fixture-model","prompt":"hello"}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/embeddings".into(),
            json!({"model":"fixture-model","input":"hello"}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/responses".into(),
            json!({"model":"fixture-model","input":"hello"}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/audio/speech".into(),
            json!({"model":"fixture-model","input":"hello","voice":"alloy"}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/images/generations".into(),
            json!({"model":"fixture-model","prompt":"an image","n":1}),
            keys[0].api_key.key.clone(),
            false,
        ),
        (
            "/v1/messages".into(),
            json!({"model":"fixture-model","max_tokens":20,"messages":[{"role":"user","content":"hello"}]}),
            keys[0].api_key.key.clone(),
            true,
        ),
    ];
    for path in [
        "/api/dashboard/playground/chat/completions".to_string(),
        "/api/dashboard/playground/load-test/chat/completions".into(),
        format!("/api/endpoints/{}/chat/completions", endpoint.id),
    ] {
        calls.push((
            path,
            json!({"model":"fixture-model","messages":[{"role":"user","content":"hello"}]}),
            jwt.clone(),
            false,
        ));
    }
    calls.push(("/v1/chat/completions".into(),json!({"model":"fixture-model","messages":[{"role":"user","content":"second key"}],"organization_id":Uuid::new_v4()}),keys[1].api_key.key.clone(),false));
    for (path, payload, key, anthropic) in calls {
        let response = app
            .clone()
            .oneshot(request(
                &path,
                &key,
                "application/json",
                serde_json::to_vec(&payload).unwrap(),
                anthropic,
            ))
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    for (path, field, prompt) in [
        ("/v1/audio/transcriptions", "file", false),
        ("/v1/images/edits", "image", true),
        ("/v1/images/variations", "image", false),
    ] {
        let response = app
            .clone()
            .oneshot(request(
                path,
                &keys[0].api_key.key,
                "multipart/form-data; boundary=receipt-boundary",
                multipart(field, prompt),
                false,
            ))
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    storage.drain_usage_writers().await.unwrap();
    let rows = sqlx::query(
        "SELECT request_id,account_id,organization_id,finalized_at FROM usage_receipts",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        14,
        "13 production route patterns plus a second key must each have one receipt"
    );
    for row in rows {
        assert!(!Uuid::parse_str(&row.get::<String, _>("request_id"))
            .unwrap()
            .is_nil());
        assert_eq!(row.get::<String, _>("account_id"), consumer.id.to_string());
        assert_eq!(row.get::<String, _>("organization_id"), org.id.to_string());
        assert!(row.get::<Option<String>, _>("finalized_at").is_some());
    }
    let raw = storage.load_records().await.unwrap();
    assert_eq!(raw.len(), 14);
    assert!(raw
        .iter()
        .all(|record| record.client_ip == Some("192.0.2.37".parse().unwrap())));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT SUM(request_count) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        14
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT SUM(total_tokens) FROM usage_daily")
            .fetch_one(&pool)
            .await
            .unwrap(),
        14 * 7,
        "provider totals must survive wire-format conversion without becoming missing"
    );
    let invalid = app
        .clone()
        .oneshot(request(
            "/v1/chat/completions",
            &keys[0].api_key.key,
            "application/json",
            b"{}".to_vec(),
            false,
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let _ = to_bytes(invalid.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        14
    );
}
