//! Issue #887: shared model probe and exact engine identities.

use reqwest::Client;
use serde_json::{json, Value};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// Issue #887: isolate exact request counts from unrelated localhost probes.
async fn detection_fixture(
    owner: &str,
    server_header: Option<&str>,
) -> (MockServer, String, Client) {
    let mock = MockServer::start().await;
    let prefix = format!("/{}", uuid::Uuid::new_v4());
    let mut response = ResponseTemplate::new(200).set_body_json(json!({
        "object": "list",
        "data": [{"id": "test-model", "object": "model", "owned_by": owner}]
    }));
    if let Some(header) = server_header {
        response = response.insert_header("server", header);
    }
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/v1/models")))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer detection-key",
        ))
        .respond_with(response)
        .expect(1)
        .mount(&mock)
        .await;
    let base_url = format!("{}{prefix}", mock.uri());
    (mock, base_url, Client::new())
}

async fn assert_detected_engine(owner: &str, probe: Option<(&str, Value)>, expected: &str) {
    let (mock, base_url, client) = detection_fixture(owner, None).await;
    if let Some((probe_path, body)) = probe {
        let prefix = base_url.strip_prefix(&mock.uri()).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("{prefix}{probe_path}")))
            .and(wiremock::matchers::header(
                "authorization",
                "Bearer detection-key",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;
    }
    let result = llmlb::detection::detect_endpoint_type_with_client(
        &client,
        &base_url,
        Some("detection-key"),
    )
    .await
    .unwrap();
    assert_eq!(result.endpoint_type.as_str(), expected);
    let requests = mock.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path().ends_with("/v1/models"))
            .count(),
        1
    );
    assert!(!requests
        .iter()
        .any(|r| r.url.path().ends_with("/v1/version")));
}

#[tokio::test]
async fn detection_sglang_uses_one_authenticated_models_response() {
    assert_detected_engine("sglang", None, "sglang").await;
}

#[tokio::test]
async fn detection_lmdeploy_uses_one_authenticated_models_response() {
    assert_detected_engine("lmdeploy", None, "lmdeploy").await;
}

#[tokio::test]
async fn detection_xinference_uses_one_authenticated_models_response() {
    assert_detected_engine("xinference", None, "xinference").await;
}

#[tokio::test]
async fn detection_lemonade_uses_one_authenticated_models_response() {
    assert_detected_engine("lemonade", None, "lemonade").await;
}

#[tokio::test]
async fn detection_tgi_requires_exact_router_identity() {
    assert_detected_engine(
        "huggingface",
        Some(("/info", json!({"router": "text-generation-router"}))),
        "tgi",
    )
    .await;
}

#[tokio::test]
async fn detection_koboldcpp_requires_exact_version_identity() {
    assert_detected_engine(
        "local",
        Some((
            "/api/extra/version",
            json!({"result": "KoboldCpp", "version": "1.0"}),
        )),
        "koboldcpp",
    )
    .await;
}

#[tokio::test]
async fn detection_shared_and_partial_owners_remain_generic() {
    for owner in [
        "local",
        "system",
        "openai",
        "SGLang",
        "sglang-community",
        "my-lmdeploy",
        "xinference-server",
        "lemonade ",
        "community-vllm",
        "lmstudio-unrelated",
    ] {
        assert_detected_engine(owner, None, "openai_compatible").await;
    }
}

#[tokio::test]
async fn detection_native_probe_lookalikes_remain_generic() {
    for (probe, body) in [
        ("/info", json!({"router": "text-generation-router-proxy"})),
        ("/api/extra/version", json!({"result": "koboldcpp"})),
        (
            "/v1/version",
            json!({"server": "llama.cpp", "version": "0.3.0"}),
        ),
    ] {
        assert_detected_engine("local", Some((probe, body)), "openai_compatible").await;
    }
}

#[tokio::test]
async fn detection_existing_models_markers_share_one_response() {
    for (owner, header, expected) in [
        ("lmstudio-community", None, "lm_studio"),
        ("vllm", None, "vllm"),
        ("local", Some("vLLM/0.6.0"), "vllm"),
        ("local", Some("llama.cpp/0.3.0"), "llamacpp"),
        ("openai", None, "openai_compatible"),
    ] {
        let (_mock, base_url, client) = detection_fixture(owner, header).await;
        let result = llmlb::detection::detect_endpoint_type_with_client(
            &client,
            &base_url,
            Some("detection-key"),
        )
        .await
        .unwrap();
        assert_eq!(result.endpoint_type.as_str(), expected);
    }
}

#[tokio::test]
async fn detection_native_engines_keep_priority_over_model_owners() {
    for (probe, body, expected) in [
        ("/api/system", json!({"xllm_version": "0.1.0"}), "xllm"),
        (
            "/api/v1/models",
            json!({"models": [{"publisher": "test", "architecture": "llama", "loaded_instances": []}]}),
            "lm_studio",
        ),
        ("/api/tags", json!({"models": []}), "ollama"),
    ] {
        let mock = MockServer::start().await;
        let prefix = format!("/{}", uuid::Uuid::new_v4());
        Mock::given(method("GET"))
            .and(path(format!("{prefix}{probe}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{prefix}/v1/models")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "test", "owned_by": "sglang"}]
            })))
            .mount(&mock)
            .await;
        let result = llmlb::detection::detect_endpoint_type_with_client(
            &Client::new(),
            &format!("{}{prefix}", mock.uri()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result.endpoint_type.as_str(), expected);
    }
}

#[tokio::test]
async fn detection_unsuccessful_models_response_is_not_an_engine_identity() {
    let mock = MockServer::start().await;
    let prefix = format!("/{}", uuid::Uuid::new_v4());
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/v1/models")))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "data": [{"owned_by": "sglang"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let result = llmlb::detection::detect_endpoint_type_with_client(
        &Client::new(),
        &format!("{}{prefix}", mock.uri()),
        None,
    )
    .await;
    assert!(matches!(
        result,
        Err(llmlb::detection::DetectionError::UnsupportedType(_))
    ));
}

#[test]
fn additional_engine_types_roundtrip_with_conservative_capabilities() {
    for name in [
        "sglang",
        "lmdeploy",
        "xinference",
        "lemonade",
        "tgi",
        "koboldcpp",
    ] {
        let endpoint_type: llmlb::types::endpoint::EndpointType = name.parse().unwrap();
        assert_eq!(endpoint_type.as_str(), name);
        assert_eq!(serde_json::to_value(endpoint_type).unwrap(), json!(name));
        assert_eq!(
            serde_json::from_value::<llmlb::types::endpoint::EndpointType>(json!(name)).unwrap(),
            endpoint_type
        );
        assert!(!endpoint_type.supports_model_download());
        assert!(!endpoint_type.supports_model_delete());
        assert!(!endpoint_type.supports_model_metadata());
        assert!(endpoint_type.is_tps_trackable());
        assert_eq!(endpoint_type.recommended_inference_timeout_secs(), 120);
    }
}
