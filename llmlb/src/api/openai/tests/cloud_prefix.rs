use super::*;

#[test]
fn parse_cloud_prefixes() {
    assert_eq!(
        parse_cloud_model("openai:gpt-4o"),
        Some(("openai".to_string(), "gpt-4o".to_string()))
    );
    assert_eq!(
        parse_cloud_model("google:gemini-pro"),
        Some(("google".to_string(), "gemini-pro".to_string()))
    );
    assert_eq!(
        parse_cloud_model("ahtnorpic:claude-3"),
        Some(("anthropic".to_string(), "claude-3".to_string()))
    );
    assert_eq!(parse_cloud_model("gpt-4"), None);
    assert_eq!(parse_cloud_model("openai:"), None);
}

#[tokio::test]
#[serial]
async fn openai_prefix_requires_api_key() {
    let _guard = TEST_LOCK.lock().await;
    // Save and remove any existing API key to test error case
    let saved = std::env::var("OPENAI_API_KEY").ok();
    std::env::remove_var("OPENAI_API_KEY");
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"openai:gpt-4o","messages":[]});
    let err = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "openai:gpt-4o",
        false,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("OPENAI_API_KEY"),
        "expected error mentioning OPENAI_API_KEY, got {}",
        msg
    );

    // Restore API key if it was set
    if let Some(key) = saved {
        std::env::set_var("OPENAI_API_KEY", key);
    }
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn google_prefix_requires_api_key() {
    let _guard = TEST_LOCK.lock().await;
    // Save and remove any existing API key to test error case
    let saved = std::env::var("GOOGLE_API_KEY").ok();
    std::env::remove_var("GOOGLE_API_KEY");
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"google:gemini-pro","messages":[]});
    let err = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "google:gemini-pro",
        false,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("GOOGLE_API_KEY"),
        "expected GOOGLE_API_KEY error, got {}",
        msg
    );

    // Restore API key if it was set
    if let Some(key) = saved {
        std::env::set_var("GOOGLE_API_KEY", key);
    }
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn anthropic_prefix_requires_api_key() {
    let _guard = TEST_LOCK.lock().await;
    // Save and remove any existing API key to test error case
    let saved = std::env::var("ANTHROPIC_API_KEY").ok();
    std::env::remove_var("ANTHROPIC_API_KEY");
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"anthropic:claude-3","messages":[]});
    let err = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "anthropic:claude-3",
        false,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("ANTHROPIC_API_KEY"),
        "expected ANTHROPIC_API_KEY error, got {}",
        msg
    );

    // Restore API key if it was set
    if let Some(key) = saved {
        std::env::set_var("ANTHROPIC_API_KEY", key);
    }
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn openai_prefix_streams_via_cloud() {
    let _guard = TEST_LOCK.lock().await;
    let server = MockServer::start().await;
    let tmpl = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "text/event-stream",
        );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    std::env::set_var("OPENAI_API_KEY", "testkey");
    std::env::set_var("OPENAI_BASE_URL", server.uri());
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"openai:gpt-4o","messages":[],"stream":true});
    let resp = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "openai:gpt-4o",
        true,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("cloud stream response");
    let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("delta"));
    assert!(body_str.contains("hi"));

    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENAI_BASE_URL");
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn google_prefix_proxies_and_maps_response() {
    let _guard = TEST_LOCK.lock().await;
    let server = MockServer::start().await;
    let tmpl = ResponseTemplate::new(200).set_body_json(json!({
        "candidates": [{"content": {"parts": [{"text": "hello from gemini"}]}}]
    }));
    Mock::given(method("POST"))
        .and(path("/models/gemini-pro:generateContent"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    std::env::set_var("GOOGLE_API_KEY", "gkey");
    std::env::set_var("GOOGLE_API_BASE_URL", server.uri());
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"google:gemini-pro","messages":[{"role":"user","content":"hi"}]});
    let resp = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "google:gemini-pro",
        false,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("google mapped response");
    let bytes = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["model"].as_str().unwrap(), "google:gemini-pro");
    assert_eq!(
        v["choices"][0]["message"]["content"].as_str().unwrap(),
        "hello from gemini"
    );

    std::env::remove_var("GOOGLE_API_KEY");
    std::env::remove_var("GOOGLE_API_BASE_URL");
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn anthropic_prefix_proxies_and_maps_response() {
    let _guard = TEST_LOCK.lock().await;
    let server = MockServer::start().await;
    let tmpl = ResponseTemplate::new(200).set_body_json(json!({
            "id": "abc123",
            "model": "claude-3",
        "content": [{"text": "anthropic says hi"}]
    }));
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    std::env::set_var("ANTHROPIC_API_KEY", "akey");
    std::env::set_var("ANTHROPIC_API_BASE_URL", server.uri());
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"anthropic:claude-3","messages":[{"role":"user","content":"hi"}]});
    let resp = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "anthropic:claude-3",
        false,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("anthropic mapped response");
    let bytes = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["model"].as_str().unwrap(), "anthropic:claude-3");
    assert_eq!(
        v["choices"][0]["message"]["content"].as_str().unwrap(),
        "anthropic says hi"
    );

    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("ANTHROPIC_API_BASE_URL");
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn cloud_request_is_recorded_in_history() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempdir().expect("temp dir");
    std::env::set_var("LLMLB_DATA_DIR", temp_dir.path());

    let state = create_local_state().await;
    let server = MockServer::start().await;
    let tmpl = ResponseTemplate::new(200).set_body_json(json!({
        "id": "chatcmpl-123",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello"},
            "finish_reason": "stop"
        }]
    }));
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    std::env::set_var("OPENAI_API_KEY", "testkey");
    std::env::set_var("OPENAI_BASE_URL", server.uri());

    let payload =
        json!({"model":"openai:gpt-4o","messages":[{"role":"user","content":"hi"}],"stream":false});
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "openai:gpt-4o".into(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("cloud proxy succeeds");

    assert_eq!(response.status(), StatusCode::OK);
    sleep(Duration::from_millis(20)).await;

    let records = state
        .balancer
        .request_history
        .load_records()
        .await
        .expect("records");
    assert_eq!(records.len(), 1, "cloud request should be recorded");

    let record = &records[0];
    assert_eq!(record.model, "openai:gpt-4o");
    assert!(matches!(record.status, RecordStatus::Success));
    assert_eq!(record.request_type, RequestType::Chat);
    assert!(
        record.response_body.is_some(),
        "response should be captured"
    );

    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENAI_BASE_URL");
    std::env::remove_var("LLMLB_DATA_DIR");
}

#[tokio::test]
#[serial]
async fn cloud_request_is_listed_in_dashboard_history() {
    use axum::routing::Router;
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    let _guard = TEST_LOCK.lock().await;

    // mock cloud provider
    let server = MockServer::start().await;
    let tmpl = ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-dashboard",
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello cloud"},
            "finish_reason": "stop"
        }]
    }));
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    // lb state with temp data dir
    std::env::set_var("OPENAI_API_KEY", "testkey");
    std::env::set_var("OPENAI_BASE_URL", server.uri());
    let (state, dir) = create_state_with_tempdir().await;

    // spawn lb
    let app: Router = crate::api::create_app(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .ok();
    });

    // send cloud request
    let client = reqwest::Client::new();
    let payload = json!({"model":"openai:gpt-4o","messages":[{"role":"user","content":"hi"}]});
    let resp = client
        .post(format!("http://{addr}/v1/chat/completions"))
        .header("x-api-key", "sk_debug")
        .json(&payload)
        .send()
        .await
        .expect("send cloud request");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // wait for async save_request_record
    wait_for_history_records(&state, |records| {
        records.iter().any(|r| r.model == "openai:gpt-4o")
    })
    .await;

    // login (dashboard is JWT-only; API keys are rejected for /api/dashboard/*)
    let login_resp = client
        .post(format!("http://{addr}/api/auth/login"))
        .header("content-type", "application/json")
        .json(&json!({"username":"admin","password":"test"}))
        .send()
        .await
        .expect("login request");
    assert_eq!(login_resp.status(), reqwest::StatusCode::OK);
    let login_body: serde_json::Value = login_resp.json().await.expect("login json");
    let jwt_token = login_body["token"].as_str().expect("login token");

    // fetch dashboard history
    let history_resp = client
        .get(format!("http://{addr}/api/dashboard/request-responses"))
        .header("authorization", format!("Bearer {jwt_token}"))
        .send()
        .await
        .expect("history request");
    assert_eq!(history_resp.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = history_resp.json().await.expect("history json");
    let records = body["records"].as_array().expect("records array");
    assert!(
        records.iter().any(|r| r["model"] == "openai:gpt-4o"),
        "cloud request should be listed in history"
    );

    // cleanup env
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENAI_BASE_URL");
    std::env::remove_var("LLMLB_DATA_DIR");
    drop(dir);
}

#[tokio::test]
#[serial]
async fn non_prefixed_model_stays_on_local_path() {
    let _guard = TEST_LOCK.lock().await;
    let state = create_local_state().await;
    let payload = json!({"model":"gpt-oss-20b","messages":[]});
    let res = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "gpt-oss-20b".into(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await;
    // モデルが登録されておらず、どのノードも報告していない場合は404
    let response = res.expect("expected 404 response, not Err");
    assert_eq!(
        response.status(),
        axum::http::StatusCode::NOT_FOUND,
        "expected NOT_FOUND for unregistered model"
    );
}

#[tokio::test]
#[serial]
async fn streaming_allowed_for_cloud_prefix() {
    let _guard = TEST_LOCK.lock().await;
    // Save and remove any existing API key to test error case
    let saved = std::env::var("OPENAI_API_KEY").ok();
    std::env::remove_var("OPENAI_API_KEY");
    let (state, _dir) = create_state_with_tempdir().await;

    let payload = json!({"model":"openai:gpt-4o","messages":[],"stream":true});
    let err = proxy_openai_cloud_post(
        &state,
        "/v1/chat/completions",
        "openai:gpt-4o",
        true,
        payload,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("OPENAI_API_KEY"),
        "expected API key error (stream path), got {}",
        msg
    );

    // Restore API key if it was set
    if let Some(key) = saved {
        std::env::set_var("OPENAI_API_KEY", key);
    }
    std::env::remove_var("LLMLB_DATA_DIR");
}

// ===== cloud_virtual_node tests =====

#[test]
fn cloud_virtual_node_returns_correct_ids_for_known_providers() {
    use super::cloud_virtual_node;
    use super::UNSPECIFIED_IP;

    let (openai_id, openai_name, openai_ip) = cloud_virtual_node("openai");
    assert_eq!(
        openai_id,
        uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000c001").unwrap()
    );
    assert_eq!(openai_name, "cloud:openai");
    assert_eq!(openai_ip, UNSPECIFIED_IP);

    let (google_id, google_name, _) = cloud_virtual_node("google");
    assert_eq!(
        google_id,
        uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000c002").unwrap()
    );
    assert_eq!(google_name, "cloud:google");

    let (anthropic_id, anthropic_name, _) = cloud_virtual_node("anthropic");
    assert_eq!(
        anthropic_id,
        uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000c003").unwrap()
    );
    assert_eq!(anthropic_name, "cloud:anthropic");
}

#[test]
fn cloud_virtual_node_returns_fallback_for_unknown_provider() {
    use super::cloud_virtual_node;

    let (id, name, _) = cloud_virtual_node("unknown-provider");
    assert_eq!(
        id,
        uuid::Uuid::parse_str("00000000-0000-0000-0000-00000000c0ff").unwrap()
    );
    assert_eq!(name, "cloud:unknown-provider");
}

// ===== parse_cloud_model extended tests =====

#[test]
fn parse_cloud_model_anthropic_prefix() {
    assert_eq!(
        parse_cloud_model("anthropic:claude-3-opus"),
        Some(("anthropic".to_string(), "claude-3-opus".to_string()))
    );
}

#[test]
fn parse_cloud_model_returns_none_for_plain_model_names() {
    assert_eq!(parse_cloud_model("llama-3-8b"), None);
    assert_eq!(parse_cloud_model("mistral-7b-instruct"), None);
    assert_eq!(parse_cloud_model(""), None);
}

#[test]
fn parse_cloud_model_returns_none_for_empty_model_after_prefix() {
    assert_eq!(parse_cloud_model("openai:"), None);
    assert_eq!(parse_cloud_model("google:"), None);
    assert_eq!(parse_cloud_model("anthropic:"), None);
    assert_eq!(parse_cloud_model("ahtnorpic:"), None);
}

// ===== Additional unit tests for increased coverage =====

// --- parse_cloud_model: ahtnorpic alias ---

#[test]
fn parse_cloud_model_ahtnorpic_is_normalized_to_anthropic() {
    let result = parse_cloud_model("ahtnorpic:claude-3.5-sonnet");
    assert_eq!(
        result,
        Some(("anthropic".to_string(), "claude-3.5-sonnet".to_string()))
    );
}

#[test]
fn parse_cloud_model_unsupported_prefix_returns_none() {
    assert_eq!(parse_cloud_model("azure:gpt-4"), None);
    assert_eq!(parse_cloud_model("aws:bedrock-model"), None);
    assert_eq!(parse_cloud_model("cohere:command"), None);
}

#[test]
fn parse_cloud_model_prefix_is_case_sensitive() {
    // The function checks specific lowercase prefixes only
    assert_eq!(parse_cloud_model("OpenAI:gpt-4o"), None);
    assert_eq!(parse_cloud_model("GOOGLE:gemini"), None);
    assert_eq!(parse_cloud_model("Anthropic:claude"), None);
}

#[test]
fn parse_cloud_model_with_slashes_in_model_name() {
    let result = parse_cloud_model("openai:org/model-name");
    assert_eq!(
        result,
        Some(("openai".to_string(), "org/model-name".to_string()))
    );
}

#[test]
fn parse_cloud_model_with_complex_model_name() {
    let result = parse_cloud_model("google:gemini-1.5-pro-002");
    assert_eq!(
        result,
        Some(("google".to_string(), "gemini-1.5-pro-002".to_string()))
    );
}

// --- cloud_virtual_node tests ---

#[test]
fn cloud_virtual_node_ids_are_all_distinct() {
    use super::cloud_virtual_node;

    let (openai_id, _, _) = cloud_virtual_node("openai");
    let (google_id, _, _) = cloud_virtual_node("google");
    let (anthropic_id, _, _) = cloud_virtual_node("anthropic");
    let (unknown_id, _, _) = cloud_virtual_node("something-else");

    assert_ne!(openai_id, google_id);
    assert_ne!(openai_id, anthropic_id);
    assert_ne!(openai_id, unknown_id);
    assert_ne!(google_id, anthropic_id);
    assert_ne!(google_id, unknown_id);
    assert_ne!(anthropic_id, unknown_id);
}

#[test]
fn cloud_virtual_node_ip_is_always_unspecified() {
    use super::cloud_virtual_node;
    use super::UNSPECIFIED_IP;

    for provider in &["openai", "google", "anthropic", "custom", ""] {
        let (_, _, ip) = cloud_virtual_node(provider);
        assert_eq!(ip, UNSPECIFIED_IP);
    }
}

#[test]
fn cloud_virtual_node_name_format() {
    use super::cloud_virtual_node;

    let (_, name, _) = cloud_virtual_node("test-provider");
    assert_eq!(name, "cloud:test-provider");

    let (_, name, _) = cloud_virtual_node("");
    assert_eq!(name, "cloud:");
}
