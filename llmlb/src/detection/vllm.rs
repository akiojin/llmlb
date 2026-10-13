//! vLLM Endpoint Type Detection
//!
//! SPEC-e8e9326e: vLLM detection via Server header
//!
//! vLLM endpoints typically include "vllm" in the Server response header
//! or respond to /v1/models in a specific way.

use super::models_response::ModelsResponse;
use reqwest::Client;
use tracing::debug;

/// Detect vLLM endpoint by checking Server header
///
/// Returns a reason string if the endpoint returns
/// a Server header containing "vllm" (case-insensitive).
pub async fn detect_vllm(client: &Client, base_url: &str, api_key: Option<&str>) -> Option<String> {
    detect_vllm_from_models(&ModelsResponse::fetch(client, base_url, api_key).await)
}

pub(super) fn detect_vllm_from_models(models: &ModelsResponse) -> Option<String> {
    if let Some(server) = &models.server {
        if server.to_ascii_lowercase().contains("vllm") {
            debug!(server_header = %server, "Detected vLLM endpoint via Server header");
            return Some(format!("vLLM: Server header contains vllm ({server})"));
        }
    }
    if models.owners().any(|owner| owner == "vllm") {
        return Some("vLLM: owned_by field is vllm".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn test_vllm_server_header_detection_logic() {
        // Test case-insensitive matching
        let server_variants = ["vLLM", "vllm", "VLLM", "vLLM/0.4.0", "server-vllm"];

        for variant in server_variants {
            assert!(
                variant.to_lowercase().contains("vllm"),
                "Should detect vLLM in: {}",
                variant
            );
        }
    }

    #[test]
    fn test_non_vllm_server_headers() {
        let non_vllm = ["nginx", "Apache", "uvicorn", "gunicorn"];

        for header in non_vllm {
            assert!(
                !header.to_lowercase().contains("vllm"),
                "Should not detect vLLM in: {}",
                header
            );
        }
    }

    #[tokio::test]
    async fn detect_vllm_by_server_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer sk-vllm"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("server", "vLLM/0.6.0")
                    .set_body_json(serde_json::json!({"data": []})),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let detected = detect_vllm(&client, &server.uri(), Some("sk-vllm")).await;
        assert!(detected.is_some());
        assert!(detected.unwrap().contains("Server header contains vllm"));
    }

    #[tokio::test]
    async fn detect_vllm_by_owned_by_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "model-a", "owned_by": "vllm"}
                ]
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let detected = detect_vllm(&client, &server.uri(), None).await;
        assert_eq!(detected, Some("vLLM: owned_by field is vllm".to_string()));
    }

    #[tokio::test]
    async fn detect_vllm_returns_none_when_not_matched() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "model-a", "owned_by": "openai"}
                ]
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        assert_eq!(detect_vllm(&client, &server.uri(), None).await, None);
    }
}
