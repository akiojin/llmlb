//! Ollama Model Download
//!
//! POST {base_url}/api/pull to download models on Ollama endpoints

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::DownloadError;

/// Ollama pull request body
#[derive(Debug, Serialize)]
struct OllamaPullRequest {
    /// Model name (e.g., "gpt-oss:20b")
    name: String,
    /// Whether to stream progress updates
    stream: bool,
}

/// Ollama pull response (non-streaming)
#[derive(Debug, Deserialize)]
struct OllamaPullResponse {
    /// Status message
    #[serde(default)]
    status: Option<String>,
}

/// Pull (download) a model on an Ollama endpoint
///
/// Sends POST /api/pull with stream=false (blocking).
/// The call blocks until the download is complete or fails.
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - Ollama endpoint base URL
/// * `model_name` - Model name to pull (e.g., "llama3.2:1b")
pub async fn pull_model(
    client: &Client,
    base_url: &str,
    model_name: &str,
) -> Result<(), DownloadError> {
    let url = format!("{}/api/pull", base_url.trim_end_matches('/'));

    let request = OllamaPullRequest {
        name: model_name.to_string(),
        stream: false,
    };

    // Ollama pull can take a long time (downloading GBs), use a generous timeout
    let response = client
        .post(&url)
        .json(&request)
        .timeout(Duration::from_secs(3600)) // 1 hour max
        .send()
        .await
        .map_err(DownloadError::Http)?;

    let status = response.status();

    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(DownloadError::EndpointError {
            status: status.as_u16(),
            message,
        });
    }

    // Parse response to check for errors
    let resp: OllamaPullResponse = response.json().await.map_err(|e| {
        DownloadError::InvalidResponse(format!("Failed to parse Ollama pull response: {}", e))
    })?;

    // Check if the response indicates success
    if let Some(status_msg) = &resp.status {
        if status_msg.contains("error") {
            return Err(DownloadError::EndpointError {
                status: 500,
                message: status_msg.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_pull_request_serialization() {
        let req = OllamaPullRequest {
            name: "llama3.2:1b".to_string(),
            stream: false,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"name\":\"llama3.2:1b\""));
        assert!(json.contains("\"stream\":false"));
    }

    #[test]
    fn test_ollama_pull_request_stream_true() {
        let req = OllamaPullRequest {
            name: "gpt-oss:20b".to_string(),
            stream: true,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"stream\":true"));
    }

    #[test]
    fn test_ollama_pull_response_deserialization() {
        let json = r#"{"status": "success"}"#;
        let resp: OllamaPullResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, Some("success".to_string()));
    }

    #[test]
    fn test_ollama_pull_response_empty() {
        let json = r#"{}"#;
        let resp: OllamaPullResponse = serde_json::from_str(json).unwrap();
        assert!(resp.status.is_none());
    }
}
