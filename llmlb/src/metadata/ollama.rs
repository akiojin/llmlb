//! Ollama Model Metadata Retrieval
//!
//! SPEC-e8e9326e: Fetch model metadata from Ollama endpoints via POST /api/show

use super::{MetadataError, ModelMetadata};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Ollama show request
#[derive(Debug, Serialize)]
struct OllamaShowRequest {
    model: String,
}

/// Ollama show response
#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    /// Model name
    #[serde(alias = "name")]
    model: Option<String>,

    /// Model details
    details: Option<OllamaModelDetails>,

    /// Model parameters (contains num_ctx)
    #[serde(alias = "model_info")]
    parameters: Option<OllamaParameters>,
}

/// Ollama model details
#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    /// Model family
    family: Option<String>,

    /// Parameter size (e.g., "7B")
    parameter_size: Option<String>,

    /// Quantization level
    quantization_level: Option<String>,
}

/// Ollama model parameters
#[derive(Debug, Deserialize)]
struct OllamaParameters {
    /// Context length
    #[serde(alias = "context_length")]
    num_ctx: Option<u32>,
}

/// Fetch model metadata from Ollama endpoint
///
/// Sends POST /api/show to the Ollama endpoint
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - Ollama endpoint base URL
/// * `model` - Model name to query
///
/// # Returns
/// Model metadata or error
pub async fn get_ollama_model_metadata(
    client: &Client,
    base_url: &str,
    model: &str,
) -> Result<ModelMetadata, MetadataError> {
    let url = format!("{}/api/show", base_url.trim_end_matches('/'));

    let request = OllamaShowRequest {
        model: model.to_string(),
    };

    let response = client
        .post(&url)
        .json(&request)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    let status = response.status();

    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(MetadataError::EndpointError {
            status: status.as_u16(),
            message,
        });
    }

    let info: OllamaShowResponse = response.json().await.map_err(|e| {
        MetadataError::InvalidResponse(format!("Failed to parse Ollama show response: {}", e))
    })?;

    let mut metadata = ModelMetadata {
        model: info.model.unwrap_or_else(|| model.to_string()),
        ..Default::default()
    };

    // Extract context length from parameters
    if let Some(params) = info.parameters {
        metadata.context_length = params.num_ctx;
    }

    // Extract details
    if let Some(details) = info.details {
        metadata.family = details.family;
        metadata.parameter_size = details.parameter_size;
        metadata.quantization = details.quantization_level;
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_show_response_deserialization() {
        let json = r#"{
            "model": "llama3.2:1b",
            "details": {
                "family": "llama",
                "parameter_size": "1B",
                "quantization_level": "Q4_K_M"
            },
            "parameters": {
                "num_ctx": 8192
            }
        }"#;

        let response: OllamaShowResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.model, Some("llama3.2:1b".to_string()));
        assert!(response.details.is_some());
        assert!(response.parameters.is_some());

        let details = response.details.unwrap();
        assert_eq!(details.family, Some("llama".to_string()));
        assert_eq!(details.parameter_size, Some("1B".to_string()));

        let params = response.parameters.unwrap();
        assert_eq!(params.num_ctx, Some(8192));
    }

    #[test]
    fn test_ollama_show_response_with_name_alias() {
        let json = r#"{
            "name": "mistral:7b",
            "details": {
                "family": "mistral"
            }
        }"#;

        let response: OllamaShowResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.model, Some("mistral:7b".to_string()));
    }

    #[test]
    fn test_ollama_show_response_minimal() {
        let json = r#"{}"#;

        let response: OllamaShowResponse = serde_json::from_str(json).unwrap();
        assert!(response.model.is_none());
        assert!(response.details.is_none());
        assert!(response.parameters.is_none());
    }

    #[test]
    fn test_ollama_show_request_serialization() {
        let request = OllamaShowRequest {
            model: "llama3.2:1b".to_string(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, r#"{"model":"llama3.2:1b"}"#);
    }

    #[test]
    fn test_ollama_show_response_with_model_info_alias() {
        // Test with model_info alias for parameters
        let json = r#"{
            "model": "phi3",
            "model_info": {
                "context_length": 4096
            }
        }"#;

        let response: OllamaShowResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.model, Some("phi3".to_string()));
        assert!(response.parameters.is_some());
        assert_eq!(response.parameters.unwrap().num_ctx, Some(4096));
    }
}
