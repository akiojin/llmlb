//! llama.cpp Endpoint Type Detection
//!
//! SPEC-e8e9326e: llama.cpp detection via User-Agent and /v1/version
//!
//! llama.cpp endpoints can be identified by:
//! 1. User-Agent header: llama.cpp/[version]
//! 2. GET /v1/version response: server field == "llama.cpp"

use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

/// /v1/version response structure
#[derive(Debug, Deserialize)]
struct LlamacppVersionResponse {
    /// Server identifier (should be "llama.cpp")
    server: Option<String>,
    /// Version string
    version: Option<String>,
}

/// Detect llama.cpp endpoint
///
/// Detection strategy (in priority order):
/// 1. User-Agent header: llama.cpp/[version] pattern
/// 2. GET /v1/version response: server field == "llama.cpp"
///
/// Returns a reason string if detection succeeds.
pub async fn detect_llamacpp(client: &Client, base_url: &str) -> Option<String> {
    // Strategy 1: Check User-Agent header via a simple request
    // We'll make a request to /v1/models and check the Server response header
    let url = format!("{}/v1/models", base_url);

    match client.get(&url).send().await {
        Ok(response) => {
            // Check if response Server header contains "llama.cpp"
            if let Some(server_header) = response.headers().get("server") {
                if let Ok(server_str) = server_header.to_str() {
                    if server_str.contains("llama.cpp") {
                        debug!(
                            server_header = %server_str,
                            "Detected llama.cpp endpoint via Server header"
                        );
                        return Some(format!(
                            "llama.cpp: Server header contains llama.cpp ({})",
                            server_str
                        ));
                    }
                }
            }
        }
        Err(e) => {
            debug!(error = %e, "llama.cpp Server header detection request failed");
        }
    }

    // Strategy 2: Check /v1/version endpoint
    let version_url = format!("{}/v1/version", base_url);

    match client.get(&version_url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<LlamacppVersionResponse>().await {
                Ok(version_resp) => {
                    // Check if server field is "llama.cpp"
                    if let Some(server) = &version_resp.server {
                        if server.contains("llama.cpp") || server == "llama.cpp" {
                            debug!(
                                server = %server,
                                version = ?version_resp.version,
                                "Detected llama.cpp endpoint via /v1/version"
                            );
                            return Some(format!(
                                "llama.cpp: /v1/version server field is '{}'",
                                server
                            ));
                        }
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Failed to parse /v1/version response");
                }
            }
        }
        Ok(response) => {
            debug!(
                status = %response.status(),
                "llama.cpp /v1/version: non-success status"
            );
        }
        Err(e) => {
            debug!(error = %e, "llama.cpp /v1/version request failed");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn detect_llamacpp_via_server_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("server", "llama.cpp/0.3.0")
                    .set_body_json(serde_json::json!({
                        "data": [],
                        "object": "list"
                    })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let result = detect_llamacpp(&client, &server.uri()).await;
        assert!(result.is_some());
        assert!(result.unwrap().contains("llama.cpp"));
    }

    #[tokio::test]
    async fn detect_llamacpp_via_version_endpoint() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "data": [],
                    "object": "list"
                })),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "server": "llama.cpp",
                    "version": "0.3.0"
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let result = detect_llamacpp(&client, &server.uri()).await;
        assert!(result.is_some());
        assert!(result.unwrap().contains("llama.cpp"));
    }

    #[tokio::test]
    async fn detect_llamacpp_returns_none_for_non_llamacpp() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "object": "list"
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "server": "openai",
                    "version": "1.0"
                })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let result = detect_llamacpp(&client, &server.uri()).await;
        assert!(result.is_none());
    }
}
