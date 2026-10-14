//! Validated query predicates for the aggregated OpenAI model catalog.

use crate::api::error::OpenAIError;
use crate::common::error::CommonError;
use crate::types::endpoint::SupportedAPI;
use axum::{extract::Query, http::Uri};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Default, Deserialize)]
struct ModelsQuery {
    capability: Option<String>,
    #[serde(rename = "type")]
    model_type: Option<String>,
    supported_api: Option<SupportedAPI>,
    ready: Option<bool>,
    endpoint_id: Option<Uuid>,
    owned_by: Option<String>,
    q: Option<String>,
}

#[derive(Default)]
pub(super) struct ModelFilter {
    query: ModelsQuery,
    capabilities: Vec<&'static str>,
    types: Vec<&'static str>,
}

impl ModelFilter {
    pub(super) fn from_uri(uri: &Uri) -> Result<Self, OpenAIError> {
        let Query(mut query) = Query::<ModelsQuery>::try_from_uri(uri)
            .map_err(|error| invalid_query(error.body_text()))?;
        let capabilities = parse_list(query.capability.as_deref(), "capability", |name| {
            const NAMES: &[&str] = &[
                "chat_completion",
                "completion",
                "embeddings",
                "fine_tune",
                "inference",
                "text_to_speech",
                "speech_to_text",
                "image_generation",
                "image_input",
            ];
            NAMES.iter().copied().find(|candidate| *candidate == name)
        })?;
        let types = parse_list(query.model_type.as_deref(), "type", |name| match name {
            "llm" => Some("chat_completion"),
            "tts" => Some("text_to_speech"),
            "stt" => Some("speech_to_text"),
            "image" => Some("image_generation"),
            "vision" => Some("image_input"),
            "embedding" => Some("embeddings"),
            _ => None,
        })?;
        query.q = query.q.map(|q| q.to_lowercase());
        Ok(Self {
            query,
            capabilities,
            types,
        })
    }

    pub(super) fn matches(&self, row: &Value) -> bool {
        if !self.capabilities.iter().all(|cap| has_capability(row, cap))
            || (!self.types.is_empty() && !self.types.iter().any(|cap| has_capability(row, cap)))
        {
            return false;
        }
        if self
            .query
            .supported_api
            .is_some_and(|api| !has_api(row, api.as_str()))
            || self
                .query
                .ready
                .is_some_and(|ready| row["ready"].as_bool() != Some(ready))
            || self
                .query
                .owned_by
                .as_deref()
                .is_some_and(|owner| row["owned_by"].as_str() != Some(owner))
        {
            return false;
        }
        if let Some(endpoint_id) = self.query.endpoint_id {
            if !row["endpoint_ids"].as_array().is_some_and(|ids| {
                ids.iter().any(|id| {
                    id.as_str().and_then(|id| Uuid::parse_str(id).ok()) == Some(endpoint_id)
                })
            }) {
                return false;
            }
        }
        if let Some(q) = &self.query.q {
            let contains = |value: &Value| {
                value
                    .as_str()
                    .is_some_and(|text| text.to_lowercase().contains(q))
            };
            return contains(&row["id"])
                || contains(&row["canonical_name"])
                || row["aliases"]
                    .as_array()
                    .is_some_and(|aliases| aliases.iter().any(contains));
        }
        true
    }
}

fn parse_list(
    raw: Option<&str>,
    parameter: &str,
    resolve: impl Fn(&str) -> Option<&'static str>,
) -> Result<Vec<&'static str>, OpenAIError> {
    raw.map(|raw| {
        raw.split(',')
            .map(|name| {
                resolve(name)
                    .ok_or_else(|| invalid_query(format!("Unknown {parameter} value: {name}")))
            })
            .collect()
    })
    .unwrap_or_else(|| Ok(Vec::new()))
}

fn invalid_query(message: String) -> OpenAIError {
    OpenAIError(CommonError::Validation(message).into())
}

fn has_api(row: &Value, api: &str) -> bool {
    row["supported_apis"]
        .as_array()
        .is_some_and(|apis| apis.iter().any(|value| value.as_str() == Some(api)))
}

fn has_capability(row: &Value, capability: &str) -> bool {
    if let Some(caps) = row.get("capabilities").and_then(Value::as_object) {
        return caps.get(capability).and_then(Value::as_bool) == Some(true);
    }
    // Responses is added to every endpoint row, so it cannot identify LLMs.
    let api = match capability {
        "chat_completion" | "completion" => "chat_completions",
        "embeddings" => "embeddings",
        "image_input" => "image_input",
        "image_generation" => "image_generation",
        _ => return false,
    };
    has_api(row, api)
}
