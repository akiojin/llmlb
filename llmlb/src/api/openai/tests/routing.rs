use super::*;

#[tokio::test]
#[serial]
async fn chat_image_input_routes_to_image_input_endpoint() {
    use crate::types::endpoint::{EndpointType, SupportedAPI};
    use axum::extract::{ConnectInfo, State};

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let text_only_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": {
                "message": "text-only endpoint should not receive image input"
            }
        })))
        .mount(&text_only_server)
        .await;

    let image_input_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-image-input",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "vision-ok"},
                "finish_reason": "stop"
            }]
        })))
        .mount(&image_input_server)
        .await;

    add_online_chat_endpoint_with_supported_apis(
        &state,
        "text-only-endpoint",
        text_only_server.uri(),
        "vision-model",
        60,
        EndpointType::LmStudio,
        vec![SupportedAPI::ChatCompletions],
    )
    .await;
    add_online_chat_endpoint_with_supported_apis(
        &state,
        "image-input-endpoint",
        image_input_server.uri(),
        "vision-model",
        60,
        EndpointType::LmStudio,
        vec![SupportedAPI::ChatCompletions, SupportedAPI::ImageInput],
    )
    .await;

    let response = super::chat_completions(
            ConnectInfo("127.0.0.1:0".parse::<SocketAddr>().unwrap()),
            HeaderMap::new(),
            State(state),
            None,
            Json(json!({
                "model": "vision-model",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Describe this image briefly."},
                        {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KGgo="}}
                    ]
                }],
                "stream": false
            })),
        )
        .await
        .expect("image input chat response");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("response body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
    assert_eq!(
        body["choices"][0]["message"]["content"].as_str(),
        Some("vision-ok")
    );

    assert_eq!(
        text_only_server
            .received_requests()
            .await
            .expect("text-only requests")
            .len(),
        0
    );
    assert_eq!(
        image_input_server
            .received_requests()
            .await
            .expect("image input requests")
            .len(),
        1
    );
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn direct_routing_body_read_failure_releases_active_request() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut read_buf = [0u8; 4096];
            let _ = socket.read(&mut read_buf).await;
            // Intentionally send fewer bytes than Content-Length to force body read failure.
            let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 256\r\nConnection: close\r\n\r\n{\"id\":\"truncated\"}";
            let _ = socket.write_all(response).await;
            let _ = socket.shutdown().await;
        }
    });

    let mut endpoint = Endpoint::new(
        "broken-endpoint".to_string(),
        format!("http://{addr}"),
        EndpointType::OpenaiCompatible,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "broken-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "broken-model",
        "messages": [{"role":"user","content":"hello"}]
    });
    let result = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "broken-model".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await;

    assert!(
        result.is_err(),
        "expected upstream body read failure to return error"
    );

    let snapshot = state
        .balancer
        .load_manager
        .snapshot(endpoint_id)
        .await
        .expect("snapshot");
    assert_eq!(
        snapshot.active_requests, 0,
        "active request count must be released on body read error"
    );
}

#[tokio::test]
#[serial]
async fn upstream_timeout_returns_gateway_timeout_response() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    let delayed_body = json!({
        "id": "chatcmpl-timeout",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "slow"},
            "finish_reason": "stop"
        }]
    });
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(delayed_body),
        )
        .mount(&server)
        .await;

    let endpoint_id =
        add_online_chat_endpoint(&state, "timeout-endpoint", server.uri(), "timeout-model", 1)
            .await;

    let response = proxy_openai_post(
        &state,
        json!({
            "model": "timeout-model",
            "messages": [{"role":"user","content":"hello"}]
        }),
        "/v1/chat/completions",
        "timeout-model".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("timeout should return response");

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("timeout body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("timeout json");
    let expected_message = format!(
        "Upstream request to {}/v1/chat/completions timed out after 1 seconds",
        server.uri()
    );
    assert_eq!(
        json["error"]["message"].as_str(),
        Some(expected_message.as_str())
    );
    assert_eq!(json["error"]["type"], "timeout");
    assert_eq!(json["error"]["code"], 504);

    let records = wait_for_history_records(&state, |records| !records.is_empty()).await;
    let snapshot = state
        .balancer
        .load_manager
        .snapshot(endpoint_id)
        .await
        .expect("snapshot");
    assert_eq!(snapshot.active_requests, 0);

    assert_eq!(records.len(), 1);
    assert!(matches!(records[0].status, RecordStatus::Error { .. }));
}

#[tokio::test]
#[serial]
async fn canonical_model_routes_to_alias_backed_endpoint_and_rewrites_payload() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(json!({"model": "gpt-oss:20b"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-alias",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            }
        })))
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "ollama-alias".to_string(),
        server.uri(),
        EndpointType::Ollama,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "gpt-oss:20b".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: Some("openai/gpt-oss-20b".to_string()),
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "openai/gpt-oss-20b",
        "messages": [{"role":"user","content":"hello"}]
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "openai/gpt-oss-20b".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("canonical request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn ollama_cold_start_timeout_returns_model_loading_error() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(2))
                .set_body_json(json!({
                    "id": "chatcmpl-cold-start",
                    "object": "chat.completion",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": "slow"},
                        "finish_reason": "stop"
                    }]
                })),
        )
        .mount(&server)
        .await;

    add_online_chat_endpoint_with_type(
        &state,
        "ollama-loading-endpoint",
        server.uri(),
        "qwen3:30b",
        1,
        crate::types::endpoint::EndpointType::Ollama,
    )
    .await;

    let response = proxy_openai_post(
        &state,
        json!({
            "model": "qwen3:30b",
            "messages": [{"role":"user","content":"hello"}]
        }),
        "/v1/chat/completions",
        "qwen3:30b".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("ollama cold-start timeout should return response");

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("model loading body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("model loading json");
    assert_eq!(json["error"]["type"], "model_loading");
    assert!(
        json["error"]["message"]
            .as_str()
            .expect("model loading message")
            .contains("still loading"),
        "expected model loading hint in response body"
    );

    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].url.path(), "/v1/chat/completions");
    assert_eq!(requests[1].url.path(), "/api/ps");
}

#[tokio::test]
#[serial]
async fn ollama_success_does_not_probe_load_state() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-success",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    add_online_chat_endpoint_with_type(
        &state,
        "ollama-success-endpoint",
        server.uri(),
        "qwen3:30b",
        5,
        crate::types::endpoint::EndpointType::Ollama,
    )
    .await;

    let response = proxy_openai_post(
        &state,
        json!({
            "model": "qwen3:30b",
            "messages": [{"role":"user","content":"hello"}]
        }),
        "/v1/chat/completions",
        "qwen3:30b".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("ollama success should return response");

    assert_eq!(response.status(), StatusCode::OK);

    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/chat/completions");
}

#[tokio::test]
#[serial]
async fn canonical_model_routes_to_secondary_lm_studio_alias_and_rewrites_payload() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(body_partial_json(
            json!({"model": "qwen/qwen3.5-35b-a3b:2"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-lmstudio-alias",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "ok"},
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            }
        })))
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "lmstudio-secondary-alias".to_string(),
        server.uri(),
        EndpointType::LmStudio,
    );
    endpoint.status = EndpointStatus::Online;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: "qwen/qwen3.5-35b-a3b:2".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: Some("Qwen/Qwen3.5-35B-A3B".to_string()),
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "Qwen/Qwen3.5-35B-A3B",
        "messages": [{"role":"user","content":"hello"}]
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "Qwen/Qwen3.5-35B-A3B".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("canonical request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn upstream_connect_failure_returns_bad_gateway_response() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    drop(listener);

    add_online_chat_endpoint(
        &state,
        "connect-failure-endpoint",
        format!("http://{addr}"),
        "connect-failure-model",
        5,
    )
    .await;

    let response = proxy_openai_post(
        &state,
        json!({
            "model": "connect-failure-model",
            "messages": [{"role":"user","content":"hello"}]
        }),
        "/v1/chat/completions",
        "connect-failure-model".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("connect failure should return response");

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("connect failure body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("connect failure json");
    let expected_message =
        format!("Failed to connect to upstream: http://{addr}/v1/chat/completions");
    assert_eq!(
        json["error"]["message"].as_str(),
        Some(expected_message.as_str())
    );
    assert_eq!(json["error"]["type"], "connection_error");
    assert_eq!(json["error"]["code"], 502);
}
