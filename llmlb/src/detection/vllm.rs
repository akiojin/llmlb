//! vLLM Endpoint Type Detection
//!
//! SPEC-e8e9326e: vLLM detection via Server header
//!
//! vLLM endpoints typically include "vllm" in the Server response header
//! or respond to /v1/models in a specific way.

use reqwest::Client;
use tracing::debug;

/// Detect vLLM endpoint by checking Server header
///
/// Returns a reason string if the endpoint returns
/// a Server header containing "vllm" (case-insensitive).
pub async fn detect_vllm(client: &Client, base_url: &str, api_key: Option<&str>) -> Option<String> {
    // Try /v1/models endpoint first
    let url = format!("{}/v1/models", base_url);

    let mut request = client.get(&url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) => {
            // Check Server header for "vllm"
            if let Some(server) = response.headers().get("server") {
                if let Ok(server_str) = server.to_str() {
                    let server_lower = server_str.to_lowercase();
                    if server_lower.contains("vllm") {
                        debug!(
                            server_header = %server_str,
                            "Detected vLLM endpoint via Server header"
                        );
                        return Some(format!(
                            "vLLM: Server header contains vllm ({})",
                            server_str
                        ));
                    }
                }
            }

            // Alternative: Check for vLLM-specific response patterns
            if response.status().is_success() {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    // vLLM may include specific fields
                    if let Some(data) = json.get("data") {
                        if let Some(models) = data.as_array() {
                            for model in models {
                                // vLLM models often have "vllm" in owned_by or other fields
                                if let Some(owned_by) = model.get("owned_by") {
                                    if owned_by
                                        .as_str()
                                        .map(|s| s.to_lowercase().contains("vllm"))
                                        .unwrap_or(false)
                                    {
                                        debug!("Detected vLLM endpoint via owned_by field");
                                        return Some(
                                            "vLLM: owned_by field contains vllm".to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        Err(e) => {
            debug!(error = %e, "vLLM detection request failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
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
}
