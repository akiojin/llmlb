//! System Info Retrieval Module
//!
//! SPEC-e8e9326e: Fetch GPU/device information from various endpoint types
//!
//! Different endpoint types expose system information via different APIs:
//! - xLLM: GET /api/system
//! - Ollama: GET /api/system
//! - llama.cpp: GET /slots (preferred) or GET /metrics (fallback)
//! - vLLM: Not supported
//! - OpenAI-compatible: Not supported

use crate::types::endpoint::{DeviceInfo, EndpointType};
use reqwest::Client;

pub mod llamacpp;

/// Fetch system/device information from an endpoint
///
/// Routes to the appropriate handler based on endpoint type.
/// Returns None if the endpoint type doesn't support system info retrieval
/// or if the request fails.
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - Endpoint base URL
/// * `api_key` - Optional API key
/// * `endpoint_type` - Type of the endpoint
///
/// # Returns
/// Device information or None if not available
pub async fn get_endpoint_system_info(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    endpoint_type: &EndpointType,
) -> Option<DeviceInfo> {
    match endpoint_type {
        EndpointType::Llamacpp => llamacpp::get_system_info(client, base_url, api_key).await,
        // Other endpoint types will be added as needed
        // EndpointType::Xllm => xllm::get_system_info(...).await,
        // EndpointType::Ollama => ollama::get_system_info(...).await,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_loaded() {
        // Placeholder test to ensure module compiles
        assert!(true);
    }
}
