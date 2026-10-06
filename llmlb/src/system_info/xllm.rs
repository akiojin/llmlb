//! xLLM system/device information retrieval via GET /api/system.

use crate::types::endpoint::DeviceInfo;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::debug;

#[derive(Debug, Deserialize)]
struct XllmSystemInfo {
    device: DeviceInfo,
}

/// Fetch GPU or CPU device information from an xLLM endpoint.
///
/// Returns None when the endpoint is unavailable or its response does not
/// contain valid device information.
pub async fn get_system_info(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<DeviceInfo> {
    let url = format!("{}/api/system", base_url.trim_end_matches('/'));
    let mut request = client.get(&url).timeout(Duration::from_secs(5));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<XllmSystemInfo>().await {
                Ok(info) => Some(info.device),
                Err(error) => {
                    debug!(error = %error, "Failed to parse xLLM device info");
                    None
                }
            }
        }
        Ok(response) => {
            debug!(status = %response.status(), "xLLM system info unavailable");
            None
        }
        Err(error) => {
            debug!(error = %error, "xLLM system info request failed");
            None
        }
    }
}
