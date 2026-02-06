//! Endpoint Type Detection Module
//!
//! SPEC-66555000: Automatic endpoint type detection
//!
//! Detection priority: xLLM > Ollama > vLLM > OpenAI-compatible

mod ollama;
mod vllm;
mod xllm;

use std::time::Duration;

use reqwest::Client;
use tracing::{debug, warn};

use crate::types::endpoint::EndpointType;

pub use ollama::detect_ollama;
pub use vllm::detect_vllm;
pub use xllm::detect_xllm;

/// Default timeout for detection requests
const DETECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Endpoint type detection result
#[derive(Debug, Clone)]
pub struct EndpointTypeDetection {
    /// 判定されたエンドポイントタイプ
    pub endpoint_type: EndpointType,
    /// 判定理由（取得できない場合はNone）
    pub reason: Option<String>,
}

impl EndpointTypeDetection {
    /// 判定結果を構築する
    pub fn new(endpoint_type: EndpointType, reason: Option<String>) -> Self {
        Self {
            endpoint_type,
            reason,
        }
    }

    /// 判別不能な場合のフォールバック
    pub fn unknown() -> Self {
        Self::new(EndpointType::Unknown, None)
    }
}

/// Detect endpoint type automatically
///
/// Tries detection in priority order:
/// 1. xLLM (GET /api/system - xllm_version field)
/// 2. Ollama (GET /api/tags)
/// 3. vLLM (Server header check)
/// 4. OpenAI-compatible (GET /v1/models)
/// 5. Unknown (fallback)
pub async fn detect_endpoint_type(base_url: &str, api_key: Option<&str>) -> EndpointTypeDetection {
    let client = Client::builder()
        .timeout(DETECTION_TIMEOUT)
        .build()
        .unwrap_or_default();

    detect_endpoint_type_with_client(&client, base_url, api_key).await
}

/// Detect endpoint type with a provided HTTP client
pub async fn detect_endpoint_type_with_client(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> EndpointTypeDetection {
    let base_url = base_url.trim_end_matches('/');

    debug!(base_url = %base_url, "Starting endpoint type detection");

    // Priority 1: xLLM detection
    if let Some(reason) = detect_xllm(client, base_url, api_key).await {
        debug!(endpoint_type = "xllm", "Detected xLLM endpoint");
        return EndpointTypeDetection::new(EndpointType::Xllm, Some(reason));
    }

    // Priority 2: Ollama detection
    if let Some(reason) = detect_ollama(client, base_url).await {
        debug!(endpoint_type = "ollama", "Detected Ollama endpoint");
        return EndpointTypeDetection::new(EndpointType::Ollama, Some(reason));
    }

    // Priority 3: vLLM detection
    if let Some(reason) = detect_vllm(client, base_url, api_key).await {
        debug!(endpoint_type = "vllm", "Detected vLLM endpoint");
        return EndpointTypeDetection::new(EndpointType::Vllm, Some(reason));
    }

    // Priority 4: OpenAI-compatible detection
    if let Some(reason) = detect_openai_compatible(client, base_url, api_key).await {
        debug!(
            endpoint_type = "openai_compatible",
            "Detected OpenAI-compatible endpoint"
        );
        return EndpointTypeDetection::new(EndpointType::OpenaiCompatible, Some(reason));
    }

    // Fallback: Unknown
    warn!(base_url = %base_url, "Could not detect endpoint type, returning Unknown");
    EndpointTypeDetection::unknown()
}

/// Detect OpenAI-compatible endpoint (GET /v1/models)
async fn detect_openai_compatible(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<String> {
    let url = format!("{}/v1/models", base_url);

    let mut request = client.get(&url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            // Check if the response looks like OpenAI models response
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if json.get("data").is_some() || json.get("object").is_some() {
                    return Some("OpenAI-compatible: /v1/models responded 200".to_string());
                }
            }
            None
        }
        Ok(_) => None,
        Err(e) => {
            debug!(error = %e, "OpenAI-compatible detection failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_timeout_is_reasonable() {
        assert!(DETECTION_TIMEOUT.as_secs() >= 3);
        assert!(DETECTION_TIMEOUT.as_secs() <= 10);
    }
}
