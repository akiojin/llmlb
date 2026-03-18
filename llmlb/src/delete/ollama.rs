//! Ollama Model Delete
//!
//! DELETE {base_url}/api/delete to remove models from Ollama endpoints

use reqwest::Client;
use serde::Serialize;
use std::time::Duration;

use super::DeleteError;

/// Ollama delete request body
#[derive(Debug, Serialize)]
struct OllamaDeleteRequest {
    /// Model name to delete (e.g., "gpt-oss:20b")
    name: String,
}

/// Delete a model from an Ollama endpoint
///
/// Sends DELETE /api/delete with a JSON body containing the model name.
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - Ollama endpoint base URL
/// * `model_name` - Model name to delete
pub async fn delete_model(
    client: &Client,
    base_url: &str,
    model_name: &str,
) -> Result<(), DeleteError> {
    let url = format!("{}/api/delete", base_url.trim_end_matches('/'));

    let request = OllamaDeleteRequest {
        name: model_name.to_string(),
    };

    let response = client
        .delete(&url)
        .json(&request)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(DeleteError::Http)?;

    let status = response.status();

    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(DeleteError::EndpointError {
            status: status.as_u16(),
            message,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_delete_request_serialization() {
        let req = OllamaDeleteRequest {
            name: "gpt-oss:20b".to_string(),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"name":"gpt-oss:20b"}"#);
    }

    #[test]
    fn test_ollama_delete_request_with_tag() {
        let req = OllamaDeleteRequest {
            name: "llama3.2:1b".to_string(),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"llama3.2:1b\""));
    }
}
