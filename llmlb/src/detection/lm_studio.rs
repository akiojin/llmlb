//! LM Studio Endpoint Type Detection
//!
//! SPEC-66555000: LM Studio detection via /api/v1/models and /v1/models
//!
//! LM Studio endpoints expose /api/v1/models with LM Studio-specific fields
//! (publisher, arch, state) and may include "lm-studio" in Server header
//! or owned_by fields on /v1/models.

use reqwest::Client;
use tracing::debug;

/// Detect LM Studio endpoint
///
/// Detection strategy (in order):
/// 1. Primary: GET /api/v1/models - check for LM Studio-specific fields (publisher/arch/state)
/// 2. Fallback 1: GET /v1/models - check Server header for "lm-studio"/"lm studio"
/// 3. Fallback 2: Same /v1/models response - check owned_by for "lm-studio"
pub async fn detect_lm_studio(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<String> {
    // Primary: Try /api/v1/models (LM Studio-specific endpoint)
    let api_url = format!("{}/api/v1/models", base_url);

    let mut request = client.get(&api_url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if has_lm_studio_fields(&json) {
                    debug!("Detected LM Studio endpoint via /api/v1/models fields");
                    return Some("LM Studio: /api/v1/models returned LM Studio format".to_string());
                }
            }
        }
        Ok(response) => {
            debug!(
                status = %response.status(),
                "LM Studio /api/v1/models: non-success status"
            );
        }
        Err(e) => {
            debug!(error = %e, "LM Studio /api/v1/models request failed");
        }
    }

    // Fallback: Try /v1/models (shared endpoint, check header and owned_by)
    let v1_url = format!("{}/v1/models", base_url);

    let mut request = client.get(&v1_url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) => {
            // Fallback 1: Check Server header for "lm-studio" or "lm studio"
            if let Some(server) = response.headers().get("server") {
                if let Ok(server_str) = server.to_str() {
                    if is_lm_studio_server_header(server_str) {
                        debug!(
                            server_header = %server_str,
                            "Detected LM Studio endpoint via Server header"
                        );
                        return Some(format!(
                            "LM Studio: Server header contains lm-studio ({})",
                            server_str
                        ));
                    }
                }
            }

            // Fallback 2: Check owned_by in response data
            if response.status().is_success() {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    if has_lm_studio_owned_by(&json) {
                        debug!("Detected LM Studio endpoint via owned_by field");
                        return Some("LM Studio: owned_by field contains lm-studio".to_string());
                    }
                }
            }

            None
        }
        Err(e) => {
            debug!(error = %e, "LM Studio /v1/models request failed");
            None
        }
    }
}

/// Check if JSON response from /api/v1/models contains LM Studio-specific fields
fn has_lm_studio_fields(json: &serde_json::Value) -> bool {
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for model in data {
            let has_publisher = model.get("publisher").is_some();
            let has_arch = model.get("arch").is_some();
            let has_state = model.get("state").is_some();
            if has_publisher && has_arch && has_state {
                return true;
            }
        }
    }
    false
}

/// Check if Server header indicates LM Studio (case-insensitive)
fn is_lm_studio_server_header(header: &str) -> bool {
    let lower = header.to_lowercase();
    lower.contains("lm-studio") || lower.contains("lm studio")
}

/// Check if any model in data array has owned_by containing "lm-studio" (case-insensitive)
fn has_lm_studio_owned_by(json: &serde_json::Value) -> bool {
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        for model in data {
            if let Some(owned_by) = model.get("owned_by").and_then(|v| v.as_str()) {
                if owned_by.to_lowercase().contains("lm-studio") {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- LM Studio JSON field detection tests ---

    #[test]
    fn test_lm_studio_fields_detected() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"object":"list","data":[{"id":"meta-llama-3.1-8b-instruct","object":"model","type":"llm","publisher":"lmstudio-community","arch":"llama","compatibility_type":"gguf","quantization":"Q4_K_M","state":"not-loaded","max_context_length":131072}]}"#
        ).unwrap();
        assert!(has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_multiple_models() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"data":[{"id":"m1","publisher":"pub","arch":"llama","state":"loaded"},{"id":"m2","publisher":"pub2","arch":"mistral","state":"not-loaded"}]}"#
        ).unwrap();
        assert!(has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_missing_publisher() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","arch":"llama","state":"loaded"}]}"#)
                .unwrap();
        assert!(!has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_missing_arch() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","publisher":"pub","state":"loaded"}]}"#)
                .unwrap();
        assert!(!has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_missing_state() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","publisher":"pub","arch":"llama"}]}"#)
                .unwrap();
        assert!(!has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_empty_data() {
        let json: serde_json::Value = serde_json::from_str(r#"{"data":[]}"#).unwrap();
        assert!(!has_lm_studio_fields(&json));
    }

    #[test]
    fn test_lm_studio_fields_no_data() {
        let json: serde_json::Value = serde_json::from_str(r#"{"object":"list"}"#).unwrap();
        assert!(!has_lm_studio_fields(&json));
    }

    // --- Server header matching tests ---

    #[test]
    fn test_server_header_lm_studio_variants() {
        let positive = [
            "LM-Studio",
            "lm-studio",
            "LM-STUDIO",
            "lm-studio/0.3.5",
            "LM Studio",
            "lm studio",
        ];
        for header in positive {
            assert!(
                is_lm_studio_server_header(header),
                "Should detect LM Studio in: {}",
                header
            );
        }
    }

    #[test]
    fn test_server_header_non_lm_studio() {
        let negative = ["nginx", "Apache", "uvicorn", "gunicorn", "vLLM/0.4.0"];
        for header in negative {
            assert!(
                !is_lm_studio_server_header(header),
                "Should not detect LM Studio in: {}",
                header
            );
        }
    }

    // --- owned_by matching tests ---

    #[test]
    fn test_owned_by_lm_studio() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","owned_by":"lm-studio"}]}"#).unwrap();
        assert!(has_lm_studio_owned_by(&json));
    }

    #[test]
    fn test_owned_by_lm_studio_case_insensitive() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","owned_by":"LM-Studio"}]}"#).unwrap();
        assert!(has_lm_studio_owned_by(&json));
    }

    #[test]
    fn test_owned_by_not_lm_studio() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","owned_by":"openai"}]}"#).unwrap();
        assert!(!has_lm_studio_owned_by(&json));
    }

    #[test]
    fn test_owned_by_no_field() {
        let json: serde_json::Value = serde_json::from_str(r#"{"data":[{"id":"m1"}]}"#).unwrap();
        assert!(!has_lm_studio_owned_by(&json));
    }

    // --- False positive tests ---

    #[test]
    fn test_vllm_headers_do_not_match() {
        assert!(!is_lm_studio_server_header("vLLM/0.4.0"));
        assert!(!is_lm_studio_server_header("vllm"));
    }

    #[test]
    fn test_standard_openai_without_lm_studio_fields() {
        // Standard OpenAI /v1/models response without publisher/arch/state
        let json: serde_json::Value = serde_json::from_str(
            r#"{"object":"list","data":[{"id":"gpt-4","object":"model","created":1687882411,"owned_by":"openai"}]}"#
        ).unwrap();
        assert!(!has_lm_studio_fields(&json));
        assert!(!has_lm_studio_owned_by(&json));
    }

    #[test]
    fn test_vllm_owned_by_does_not_match() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"data":[{"id":"m1","owned_by":"vllm"}]}"#).unwrap();
        assert!(!has_lm_studio_owned_by(&json));
    }
}
