//! Ollama Endpoint Type Detection
//!
//! SPEC-e8e9326e: Ollama detection via GET /api/tags
//!
//! Ollama endpoints expose a /api/tags endpoint that returns
//! a list of available models in Ollama-specific format.

use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

/// Ollama tags response structure
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    /// List of available models
    models: Option<Vec<OllamaModel>>,
}

/// Ollama model info (minimal)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaModel {
    /// Model name (e.g., "llama3:8b")
    name: String,
    /// Model size in bytes
    #[serde(default)]
    size: Option<i64>,
}

/// Detect Ollama endpoint by querying GET /api/tags
///
/// Returns a reason string if the endpoint responds with
/// a JSON object containing `models` array (Ollama-specific format).
pub async fn detect_ollama(client: &Client, base_url: &str) -> Option<String> {
    let url = format!("{}/api/tags", base_url);

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<OllamaTagsResponse>().await {
                Ok(tags) => {
                    // Ollama returns { "models": [...] }
                    if tags.models.is_some() {
                        debug!(
                            model_count = tags.models.as_ref().map(|m| m.len()),
                            "Detected Ollama endpoint"
                        );
                        return Some("Ollama: /api/tags returned models".to_string());
                    }
                    None
                }
                Err(e) => {
                    debug!(error = %e, "Failed to parse Ollama tags response");
                    None
                }
            }
        }
        Ok(response) => {
            debug!(
                status = %response.status(),
                "Ollama detection: non-success status"
            );
            None
        }
        Err(e) => {
            debug!(error = %e, "Ollama detection request failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_tags_response_deserialize() {
        let json = r#"{"models": [{"name": "llama3:8b", "size": 4000000000}]}"#;
        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert!(response.models.is_some());
        let models = response.models.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama3:8b");
    }

    #[test]
    fn test_ollama_tags_response_empty_models() {
        let json = r#"{"models": []}"#;
        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert!(response.models.is_some());
        assert!(response.models.unwrap().is_empty());
    }

    #[test]
    fn test_ollama_tags_response_no_models_field() {
        let json = r#"{}"#;
        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert!(response.models.is_none());
    }
}
