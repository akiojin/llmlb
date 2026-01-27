//! xLLM Model Download Client
//!
//! SPEC-66555000: Model download request and progress tracking for xLLM endpoints

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Error types for xLLM download operations
#[derive(Debug, Error)]
pub enum DownloadError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// xLLM returned an error response
    #[error("xLLM returned error: {status} - {message}")]
    XllmError {
        /// HTTP status code
        status: u16,
        /// Error message
        message: String,
    },

    /// Invalid response format from xLLM
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

/// Request to download a model from HuggingFace
#[derive(Debug, Clone, Serialize)]
pub struct DownloadRequest {
    /// HuggingFace model repository (e.g., "bartowski/Llama-3.2-1B-Instruct-GGUF")
    pub repo: String,

    /// Optional filename to download (e.g., "Llama-3.2-1B-Instruct-Q4_K_M.gguf")
    /// If not specified, xLLM will choose the best quantization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// Response from xLLM download progress endpoint
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadProgressResponse {
    /// Download task ID
    pub task_id: String,

    /// Model being downloaded
    pub model: String,

    /// Current status: "pending", "downloading", "completed", "failed", "cancelled"
    pub status: String,

    /// Download progress (0.0 - 100.0)
    pub progress: f64,

    /// Download speed in MB/s (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mbps: Option<f64>,

    /// Estimated time remaining in seconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u32>,

    /// Error message if status is "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Downloaded filename (available when completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

/// xLLM download initiation response
#[derive(Debug, Clone, Deserialize)]
struct DownloadInitResponse {
    task_id: String,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    status: String,
}

/// Request model download from xLLM endpoint
///
/// Sends POST /v0/models/download to the xLLM endpoint
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - xLLM endpoint base URL
/// * `api_key` - Optional API key for authentication
/// * `request` - Download request details
///
/// # Returns
/// Task ID for tracking download progress
pub async fn download_model(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    request: &DownloadRequest,
) -> Result<String, DownloadError> {
    let url = format!("{}/v0/models/download", base_url.trim_end_matches('/'));

    let mut req_builder = client
        .post(&url)
        .json(request)
        .timeout(Duration::from_secs(30));

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder.send().await?;
    let status = response.status();

    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(DownloadError::XllmError {
            status: status.as_u16(),
            message,
        });
    }

    let init_response: DownloadInitResponse = response.json().await.map_err(|e| {
        DownloadError::InvalidResponse(format!("Failed to parse download response: {}", e))
    })?;

    Ok(init_response.task_id)
}

/// Get download progress from xLLM endpoint
///
/// Sends GET /v0/download/progress?task_id={task_id} to the xLLM endpoint
///
/// # Arguments
/// * `client` - HTTP client
/// * `base_url` - xLLM endpoint base URL
/// * `api_key` - Optional API key for authentication
/// * `task_id` - Download task ID
///
/// # Returns
/// Current download progress
pub async fn get_download_progress(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    task_id: &str,
) -> Result<DownloadProgressResponse, DownloadError> {
    let url = format!(
        "{}/v0/download/progress?task_id={}",
        base_url.trim_end_matches('/'),
        task_id
    );

    let mut req_builder = client.get(&url).timeout(Duration::from_secs(10));

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder.send().await?;
    let status = response.status();

    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(DownloadError::XllmError {
            status: status.as_u16(),
            message,
        });
    }

    let progress: DownloadProgressResponse = response.json().await.map_err(|e| {
        DownloadError::InvalidResponse(format!("Failed to parse progress response: {}", e))
    })?;

    Ok(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_request_serialization() {
        let request = DownloadRequest {
            repo: "bartowski/Llama-3.2-1B-Instruct-GGUF".to_string(),
            filename: Some("Llama-3.2-1B-Instruct-Q4_K_M.gguf".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("bartowski/Llama-3.2-1B-Instruct-GGUF"));
        assert!(json.contains("Llama-3.2-1B-Instruct-Q4_K_M.gguf"));
    }

    #[test]
    fn test_download_request_without_filename() {
        let request = DownloadRequest {
            repo: "bartowski/Llama-3.2-1B-Instruct-GGUF".to_string(),
            filename: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("bartowski/Llama-3.2-1B-Instruct-GGUF"));
        assert!(!json.contains("filename"));
    }

    #[test]
    fn test_download_progress_deserialization() {
        let json = r#"{
            "task_id": "task-123",
            "model": "llama-3.2-1b",
            "status": "downloading",
            "progress": 45.5,
            "speed_mbps": 12.3,
            "eta_seconds": 120
        }"#;

        let progress: DownloadProgressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(progress.task_id, "task-123");
        assert_eq!(progress.status, "downloading");
        assert_eq!(progress.progress, 45.5);
        assert_eq!(progress.speed_mbps, Some(12.3));
        assert_eq!(progress.eta_seconds, Some(120));
    }

    #[test]
    fn test_download_progress_completed() {
        let json = r#"{
            "task_id": "task-123",
            "model": "llama-3.2-1b",
            "status": "completed",
            "progress": 100.0,
            "filename": "model.gguf"
        }"#;

        let progress: DownloadProgressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(progress.status, "completed");
        assert_eq!(progress.progress, 100.0);
        assert_eq!(progress.filename, Some("model.gguf".to_string()));
    }

    #[test]
    fn test_download_progress_failed() {
        let json = r#"{
            "task_id": "task-123",
            "model": "llama-3.2-1b",
            "status": "failed",
            "progress": 25.0,
            "error": "Connection timeout"
        }"#;

        let progress: DownloadProgressResponse = serde_json::from_str(json).unwrap();
        assert_eq!(progress.status, "failed");
        assert_eq!(progress.error, Some("Connection timeout".to_string()));
    }

    #[test]
    fn test_download_error_display() {
        let err = DownloadError::XllmError {
            status: 404,
            message: "Model not found".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "xLLM returned error: 404 - Model not found"
        );
    }
}
