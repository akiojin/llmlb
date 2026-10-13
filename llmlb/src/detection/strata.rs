//! Strata / Project Maya detection via their unauthenticated GET /health.

use super::DetectionResult;
use crate::types::endpoint::EndpointType;
use reqwest::Client;
use serde_json::Value;

/// Identify the family with one public health probe, without relying on /props.
pub(super) async fn detect_strata_family(
    client: &Client,
    base_url: &str,
) -> Option<DetectionResult> {
    let response = client.get(format!("{base_url}/health")).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let health: Value = response.json().await.ok()?;

    // Maya is a Strata derivative: its marker wins even if both are present.
    // Presence matters because mcp_tools is false when MCP tools are disabled.
    if health.get("mcp_tools").is_some() {
        return Some(DetectionResult {
            endpoint_type: EndpointType::ProjectMaya,
            reason: "Project Maya: /health contains mcp_tools".to_string(),
        });
    }
    if health.get("service").and_then(Value::as_str) == Some("strata") {
        return Some(DetectionResult {
            endpoint_type: EndpointType::Strata,
            reason: "Strata: /health service is strata".to_string(),
        });
    }
    None
}
