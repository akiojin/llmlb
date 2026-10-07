use super::*;

#[tokio::test]
#[serial]
async fn local_streaming_request_updates_model_tps_after_stream_completion() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    let stream_body = concat!(
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let tmpl = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(stream_body, "text/event-stream");
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(tmpl)
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "stream-tps-endpoint".to_string(),
        server.uri(),
        EndpointType::Vllm,
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
            model_id: "stream-tps-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "stream-tps-model",
        "messages": [{"role":"user","content":"hello"}],
        "stream": true
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "stream-tps-model".to_string(),
        true,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("streaming request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("stream body should be readable");

    sleep(Duration::from_millis(100)).await;

    let tps = state.balancer.load_manager.get_model_tps(endpoint_id).await;
    let entry = tps
        .iter()
        .find(|info| info.model_id == "stream-tps-model")
        .expect("stream model should have TPS entry");
    assert!(entry.tps.is_some(), "TPS should be updated");
    assert!(
        entry.total_output_tokens > 0,
        "streaming output tokens should be accumulated"
    );
}

/// SPEC #575 US-013-A (2026-04-23 delta):
/// llama.cpp タイプのエンドポイントに対するストリーミング chat completions が
/// 正常にプロキシされ、TPS 計測が更新されることを確認する。
#[tokio::test]
#[serial]
async fn llamacpp_streaming_request_updates_model_tps_after_stream_completion() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    let stream_body = concat!(
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(stream_body, "text/event-stream"),
        )
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "llamacpp-stream-tps-endpoint".to_string(),
        server.uri(),
        EndpointType::Llamacpp,
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
            model_id: "llamacpp-stream-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "llamacpp-stream-model",
        "messages": [{"role":"user","content":"hello"}],
        "stream": true
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "llamacpp-stream-model".to_string(),
        true,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("streaming request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("stream body should be readable");

    sleep(Duration::from_millis(100)).await;

    let tps = state.balancer.load_manager.get_model_tps(endpoint_id).await;
    let entry = tps
        .iter()
        .find(|info| info.model_id == "llamacpp-stream-model")
        .expect("llamacpp stream model should have TPS entry");
    assert!(entry.tps.is_some(), "TPS should be updated for llamacpp");
    assert!(
        entry.total_output_tokens > 0,
        "streaming output tokens should be accumulated for llamacpp"
    );
}

#[tokio::test]
#[serial]
async fn interrupted_streaming_request_still_records_success_stats() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    let stream_body = concat!(
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(stream_body, "text/event-stream"),
        )
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "stream-interrupted-endpoint".to_string(),
        server.uri(),
        EndpointType::Vllm,
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
            model_id: "stream-interrupted-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");

    let response = proxy_openai_post(
        &state,
        json!({
            "model": "stream-interrupted-model",
            "messages": [{"role":"user","content":"hello"}],
            "stream": true
        }),
        "/v1/chat/completions",
        "stream-interrupted-model".to_string(),
        true,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("streaming request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    // Simulate client disconnect before fully draining the upstream stream.
    drop(response);

    sleep(Duration::from_millis(120)).await;

    let endpoint = crate::db::endpoints::get_endpoint(&state.db_pool, endpoint_id)
        .await
        .expect("get endpoint should succeed")
        .expect("endpoint should exist");
    assert_eq!(endpoint.total_requests, 1);
    assert_eq!(endpoint.successful_requests, 1);
    assert_eq!(endpoint.failed_requests, 0);

    let model_stats = crate::db::endpoint_daily_stats::get_model_stats(&state.db_pool, endpoint_id)
        .await
        .expect("get model stats");
    let stat = model_stats
        .iter()
        .find(|s| s.model_id == "stream-interrupted-model")
        .expect("model stats should exist for interrupted stream");
    assert_eq!(stat.total_requests, 1);
    assert_eq!(stat.successful_requests, 1);
    assert_eq!(stat.failed_requests, 0);
}

#[tokio::test]
#[serial]
async fn non_stream_without_usage_does_not_accumulate_tps_duration() {
    use crate::types::endpoint::{
        Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
    };

    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    let server = MockServer::start().await;
    let body_without_usage = json!({
        "id": "chatcmpl-no-usage",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello"},
            "finish_reason": "stop"
        }]
    });
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body_without_usage))
        .mount(&server)
        .await;

    let mut endpoint = Endpoint::new(
        "no-usage-endpoint".to_string(),
        server.uri(),
        EndpointType::Vllm,
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
            model_id: "no-usage-model".to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");

    let payload = json!({
        "model": "no-usage-model",
        "messages": [{"role":"user","content":"hello"}],
        "stream": false
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "no-usage-model".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    sleep(Duration::from_millis(100)).await;

    let model_stats = crate::db::endpoint_daily_stats::get_model_stats(&state.db_pool, endpoint_id)
        .await
        .expect("get model stats");
    let stat = model_stats
        .iter()
        .find(|s| s.model_id == "no-usage-model")
        .expect("model stats should exist");
    assert_eq!(
        stat.total_output_tokens, 0,
        "usageがない場合はoutput_tokensを加算しない"
    );
    assert_eq!(
        stat.total_duration_ms, 0,
        "usageがない場合はduration_msを加算しない"
    );
}
