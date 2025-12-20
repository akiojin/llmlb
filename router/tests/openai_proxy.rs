use axum::http::{header::CONTENT_TYPE, StatusCode};
use llm_router::{api, balancer::LoadManager, registry::NodeRegistry, AppState};
use llm_router_common::types::GpuDeviceInfo;
use tower::ServiceExt;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn build_state_with_mock(mock: &MockServer) -> (AppState, String) {
    std::env::set_var("LLM_CONVERT_FAKE", "1");
    let registry = NodeRegistry::new();
    let load_manager = LoadManager::new(registry.clone());
    let db_pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run migrations");
    let request_history = std::sync::Arc::new(
        llm_router::db::request_history::RequestHistoryStorage::new(db_pool.clone()),
    );
    let convert_manager = llm_router::convert::ConvertTaskManager::new(1, db_pool.clone());
    let jwt_secret = "test-secret".to_string();
    let state = AppState {
        registry,
        load_manager,
        request_history,
        convert_manager,
        db_pool: db_pool.clone(),
        jwt_secret,
        http_client: reqwest::Client::new(),
    };

    // 登録済みノードを追加
    state
        .registry
        .register(llm_router_common::protocol::RegisterRequest {
            machine_name: "mock-node".into(),
            ip_address: mock.address().ip(),
            runtime_version: "0.0.0".into(),
            // APIポート=runtime_port+1 となる仕様のため、実際のモックポートに合わせて -1 する
            runtime_port: mock.address().port().saturating_sub(1),
            gpu_available: true,
            gpu_devices: vec![GpuDeviceInfo {
                model: "Test GPU".to_string(),
                count: 1,
                memory: None,
            }],
            gpu_count: Some(1),
            gpu_model: Some("Test GPU".to_string()),
            supported_runtimes: Vec::new(),
        })
        .await
        .unwrap();

    // ノードをready状態にしておく（初期化待ちやモデル未ロードで404/503にならないように）
    let node_id = state.registry.list().await[0].id;

    // レジストリにロード済みモデル・初期化解除を反映
    state
        .registry
        .update_last_seen(
            node_id,
            Some(vec![
                "gpt-oss-20b".to_string(),
                "gpt-oss-120b".to_string(),
                "test-model".to_string(),
            ]),
            None, // loaded_embedding_models
            None,
            None,
            None,
            Some(false),
            Some((4, 4)),
        )
        .await
        .ok();

    state
        .load_manager
        .record_metrics(llm_router::balancer::MetricsUpdate {
            node_id,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            gpu_usage: None,
            gpu_memory_usage: None,
            gpu_memory_total_mb: None,
            gpu_memory_used_mb: None,
            gpu_temperature: None,
            gpu_model_name: None,
            gpu_compute_capability: None,
            gpu_capability_score: None,
            active_requests: 0,
            average_response_time_ms: Some(1.0),
            initializing: false,
            ready_models: Some((4, 4)),
        })
        .await
        .unwrap();

    // テスト用のユーザーを作成
    let test_user = llm_router::db::users::create(
        &db_pool,
        "test-user",
        "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyWpLF5JRSia", // bcrypt hash of "password"
        llm_router_common::auth::UserRole::Admin,
    )
    .await
    .expect("Failed to create test user");

    // テスト用のAPIキーを作成
    let api_key = llm_router::db::api_keys::create(&db_pool, "test-key", test_user.id, None)
        .await
        .expect("Failed to create test API key");

    (state, api_key.key)
}

#[tokio::test]
async fn test_openai_chat_completions_success() {
    let mock_server = MockServer::start().await;

    let upstream = serde_json::json!({
        "object": "chat.completion",
        "choices": [{
            "message": { "role": "assistant", "content": "Hello from OpenAI route" },
            "finish_reason": "stop",
            "index": 0
        }]
    });

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-model",
        "messages": [
            { "role": "user", "content": "hi?" }
        ],
        "stream": false
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["object"], "chat.completion");
    assert_eq!(
        json["choices"][0]["message"]["content"],
        "Hello from OpenAI route"
    );
}

#[tokio::test]
async fn test_openai_chat_completions_streaming_passthrough() {
    let mock_server = MockServer::start().await;
    let sse_payload = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello stream\"}}]}\n\n";

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/event-stream")
                .set_body_bytes(sse_payload.as_bytes().to_vec()),
        )
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-model",
        "messages": [
            { "role": "user", "content": "stream?" }
        ],
        "stream": true
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), sse_payload);
}

#[tokio::test]
async fn test_openai_completions_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": "Generated via OpenAI route",
            "done": true
        })))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-model",
        "prompt": "say hello",
        "stream": false
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["response"], "Generated via OpenAI route");
}

#[tokio::test]
async fn test_openai_completions_streaming_passthrough() {
    let mock_server = MockServer::start().await;
    let ndjson_payload = "{\"response\":\"chunk-1\"}\n{\"response\":\"chunk-2\"}\n";

    Mock::given(method("POST"))
        .and(path("/v1/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "application/x-ndjson")
                .set_body_bytes(ndjson_payload.as_bytes().to_vec()),
        )
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-model",
        "prompt": "stream please",
        "stream": true
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(std::str::from_utf8(&body).unwrap(), ndjson_payload);
}

#[tokio::test]
async fn test_openai_chat_completions_preserves_extra_fields() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(serde_json::json!({
            "temperature": 0.5,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "hi"},
                        {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
                    ]
                }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "choices": [{
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop",
                "index": 0
            }]
        })))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-model",
        "temperature": 0.5,
        "messages": [
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": "hi" },
                    { "type": "image_url", "image_url": { "url": "data:image/png;base64,AAAA" } }
                ]
            }
        ]
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_openai_embeddings_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "index": 0,
                "object": "embedding",
                "embedding": [0.1, 0.2, 0.3]
            }],
            "model": "test-embed",
            "object": "list"
        })))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let payload = serde_json::json!({
        "model": "test-embed",
        "input": "embed me"
    });

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/embeddings")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(value["object"], "list");
}

#[tokio::test]
async fn test_openai_models_list_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [{
                "id": "gpt-oss-20b",
                "object": "model",
                "owned_by": "runtime"
            }]
        })))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        value["data"]
            .as_array()
            .map(|arr| arr.is_empty())
            .unwrap_or(true),
        "/v1/models should be empty when no downloaded models exist"
    );
}

#[tokio::test]
async fn test_openai_model_detail_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models/gpt-oss-20b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "gpt-oss-20b",
            "object": "model",
            "owned_by": "runtime"
        })))
        .mount(&mock_server)
        .await;

    let (state, api_key) = build_state_with_mock(&mock_server).await;
    let router = api::create_router(state);

    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/v1/models/gpt-oss-20b")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
