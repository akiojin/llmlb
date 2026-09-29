//! エンドポイント種別に応じたシステム情報取得の振り分け

use super::llamacpp;
use crate::types::endpoint::{DeviceInfo, EndpointType};
use reqwest::Client;

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
    fn test_module_loaded() {}
}
