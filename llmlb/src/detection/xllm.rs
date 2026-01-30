//! xLLM Endpoint Type Detection
//!
//! SPEC-66555000: xLLM detection via GET /api/system
//!
//! xLLM endpoints expose a /api/system endpoint that returns
//! system information including `xllm_version` field.

use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

use crate::types::endpoint::EndpointType;

/// xLLM system info response
#[derive(Debug, Deserialize)]
struct XllmSystemInfo {
    /// xLLM version string (e.g., "0.1.0")
    xllm_version: Option<String>,
    /// Optional server name
    #[serde(default)]
    #[allow(dead_code)]
    server_name: Option<String>,
}

/// Detect xLLM endpoint by querying GET /api/system
///
/// Returns `Some(EndpointType::Xllm)` if the endpoint responds with
/// a JSON object containing `xllm_version` field.
pub async fn detect_xllm(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<EndpointType> {
    let url = format!("{}/api/system", base_url);

    let mut request = client.get(&url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<XllmSystemInfo>().await {
                Ok(info) => {
                    if info.xllm_version.is_some() {
                        debug!(
                            version = ?info.xllm_version,
                            "Detected xLLM endpoint"
                        );
                        return Some(EndpointType::Xllm);
                    }
                    None
                }
                Err(e) => {
                    debug!(error = %e, "Failed to parse xLLM system info");
                    None
                }
            }
        }
        Ok(response) => {
            debug!(
                status = %response.status(),
                "xLLM detection: non-success status"
            );
            None
        }
        Err(e) => {
            debug!(error = %e, "xLLM detection request failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xllm_system_info_deserialize() {
        let json = r#"{"xllm_version": "0.1.0", "server_name": "test"}"#;
        let info: XllmSystemInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.xllm_version, Some("0.1.0".to_string()));
    }

    #[test]
    fn test_xllm_system_info_deserialize_minimal() {
        let json = r#"{"xllm_version": "1.0.0"}"#;
        let info: XllmSystemInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.xllm_version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_xllm_system_info_deserialize_no_version() {
        let json = r#"{"server_name": "other"}"#;
        let info: XllmSystemInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.xllm_version, None);
    }
}
