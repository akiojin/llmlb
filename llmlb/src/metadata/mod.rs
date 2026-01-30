//! Model Metadata Retrieval Module
//!
//! SPEC-66555000: Fetch model metadata (context_length, etc.) from various endpoint types

pub mod ollama;
pub mod xllm;

use crate::types::endpoint::EndpointType;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for metadata retrieval operations
#[derive(Debug, Error)]
pub enum MetadataError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Endpoint returned an error response
    #[error("Endpoint returned error: {status} - {message}")]
    EndpointError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },

    /// Invalid response format
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Metadata not available for this endpoint type
    #[error("Metadata retrieval not supported for endpoint type: {0}")]
    UnsupportedType(String),
}

/// Model metadata information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelMetadata {
    /// Model name/identifier
    pub model: String,

    /// Maximum context length (tokens)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,

    /// Model file size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,

    /// Quantization type (e.g., "Q4_K_M", "Q8_0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,

    /// Model family (e.g., "llama", "mistral")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,

    /// Parameter count (e.g., "7B", "70B")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_size: Option<String>,
}

/// Fetch model metadata from an endpoint
///
/// Routes to the appropriate handler based on endpoint type
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - Endpoint base URL
/// * `api_key` - Optional API key
/// * `endpoint_type` - Type of the endpoint
/// * `model` - Model name to query
///
/// # Returns
/// Model metadata or error
pub async fn get_model_metadata(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    endpoint_type: &EndpointType,
    model: &str,
) -> Result<ModelMetadata, MetadataError> {
    match endpoint_type {
        EndpointType::Xllm => xllm::get_xllm_model_metadata(client, base_url, api_key, model).await,
        EndpointType::Ollama => ollama::get_ollama_model_metadata(client, base_url, model).await,
        EndpointType::Vllm => {
            // vLLM doesn't have a standard metadata endpoint
            // Return minimal metadata
            Ok(ModelMetadata {
                model: model.to_string(),
                ..Default::default()
            })
        }
        EndpointType::OpenaiCompatible => {
            // OpenAI-compatible endpoints may not have metadata endpoints
            Ok(ModelMetadata {
                model: model.to_string(),
                ..Default::default()
            })
        }
        EndpointType::Unknown => Err(MetadataError::UnsupportedType("unknown".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metadata_default() {
        let metadata = ModelMetadata::default();
        assert!(metadata.model.is_empty());
        assert!(metadata.context_length.is_none());
        assert!(metadata.size_bytes.is_none());
    }

    #[test]
    fn test_model_metadata_serialization() {
        let metadata = ModelMetadata {
            model: "llama-3.2-1b".to_string(),
            context_length: Some(8192),
            size_bytes: Some(1_500_000_000),
            quantization: Some("Q4_K_M".to_string()),
            family: Some("llama".to_string()),
            parameter_size: Some("1B".to_string()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"model\":\"llama-3.2-1b\""));
        assert!(json.contains("\"context_length\":8192"));
        assert!(json.contains("\"quantization\":\"Q4_K_M\""));
    }

    #[test]
    fn test_model_metadata_deserialization() {
        let json = r#"{
            "model": "mistral-7b",
            "context_length": 4096,
            "family": "mistral"
        }"#;

        let metadata: ModelMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.model, "mistral-7b");
        assert_eq!(metadata.context_length, Some(4096));
        assert_eq!(metadata.family, Some("mistral".to_string()));
        assert!(metadata.size_bytes.is_none());
    }

    #[test]
    fn test_metadata_error_display() {
        let err = MetadataError::EndpointError {
            status: 404,
            message: "Model not found".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Endpoint returned error: 404 - Model not found"
        );
    }

    #[test]
    fn test_metadata_error_unsupported_type() {
        let err = MetadataError::UnsupportedType("unknown".to_string());
        assert_eq!(
            err.to_string(),
            "Metadata retrieval not supported for endpoint type: unknown"
        );
    }
}
