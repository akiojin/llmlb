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
    use super::*;
    use crate::types::endpoint::DeviceType;
    use serde_json::json;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn xllm_system_info_preserves_gpu_memory_and_sends_bearer_auth() {
        let server = MockServer::start().await;
        let base_path = format!("/{}", uuid::Uuid::new_v4());
        Mock::given(method("GET"))
            .and(path(format!("{base_path}/api/system")))
            .and(header("authorization", "Bearer sk-system-info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "xllm_version": "0.1.0",
                "device": {
                    "device_type": "gpu",
                    "gpu_devices": [{
                        "name": "Apple M1 Max",
                        "total_memory_bytes": 34359738368_u64,
                        "used_memory_bytes": 8589934592_u64
                    }]
                }
            })))
            .mount(&server)
            .await;

        let info = get_endpoint_system_info(
            &Client::new(),
            &format!("{}{base_path}/", server.uri()),
            Some("sk-system-info"),
            &EndpointType::Xllm,
        )
        .await
        .expect("xLLM GPU device info should be retrieved");

        assert_eq!(info.device_type, DeviceType::Gpu);
        assert_eq!(info.gpu_devices.len(), 1);
        assert_eq!(info.gpu_devices[0].name, "Apple M1 Max");
        assert_eq!(info.gpu_devices[0].total_memory_bytes, 34359738368);
        assert_eq!(info.gpu_devices[0].used_memory_bytes, 8589934592);
    }

    #[tokio::test]
    async fn xllm_system_info_accepts_cpu_without_gpu_devices() {
        let server = MockServer::start().await;
        let base_path = format!("/{}", uuid::Uuid::new_v4());
        Mock::given(method("GET"))
            .and(path(format!("{base_path}/api/system")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "xllm_version": "0.1.0",
                "device": {"device_type": "cpu"}
            })))
            .mount(&server)
            .await;

        let info = get_endpoint_system_info(
            &Client::new(),
            &format!("{}{base_path}", server.uri()),
            None,
            &EndpointType::Xllm,
        )
        .await
        .expect("CPU-only xLLM device info should be retrieved");

        assert_eq!(info.device_type, DeviceType::Cpu);
        assert!(info.gpu_devices.is_empty());
    }

    #[tokio::test]
    async fn xllm_system_info_ignores_http_errors_and_invalid_device_responses() {
        let server = MockServer::start().await;
        let client = Client::new();
        let responses = [
            ResponseTemplate::new(404),
            ResponseTemplate::new(503),
            ResponseTemplate::new(200).set_body_string("{"),
            ResponseTemplate::new(200).set_body_json(json!({"xllm_version": "0.1.0"})),
            ResponseTemplate::new(200).set_body_json(json!({"device": null})),
            ResponseTemplate::new(200).set_body_json(json!({"device": {"device_type": "unknown"}})),
            ResponseTemplate::new(200).set_body_json(json!({
                "device": {"device_type": "gpu", "gpu_devices": [{"name": "incomplete"}]}
            })),
        ];

        for response in responses {
            let base_path = format!("/{}", uuid::Uuid::new_v4());
            Mock::given(method("GET"))
                .and(path(format!("{base_path}/api/system")))
                .respond_with(response)
                .expect(1)
                .mount(&server)
                .await;

            assert!(get_endpoint_system_info(
                &client,
                &format!("{}{base_path}", server.uri()),
                None,
                &EndpointType::Xllm,
            )
            .await
            .is_none());
        }
    }

    #[test]
    fn test_module_loaded() {}
}
