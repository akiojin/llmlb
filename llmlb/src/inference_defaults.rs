//! Global defaults are read only for a missing model and become request-local state.

pub mod storage;
pub use storage::{load, save, InferenceDefaults, SETTINGS_KEY};

use crate::api::{
    model_name::{parse_quantized_model_name, resolve_runtime_model_name_for_endpoint},
    openai::parse_cloud_model,
};
use crate::common::error::{CommonError, LbError};
use crate::registry::endpoints::endpoint_model_matches;
use crate::types::{
    endpoint::{Endpoint, EndpointStatus, SupportedAPI},
    model::ModelCapability,
};
use crate::AppState;
use serde_json::Value;
use sqlx::SqlitePool;

/// The existing text-generation ingress protocols.
#[derive(Clone, Copy)]
pub(crate) enum TextRoute {
    Chat,
    LegacyCompletion,
    Responses,
    Messages,
}

/// Return whether a default was inserted. Explicit and non-object inputs never read settings.
pub(crate) async fn apply_default(pool: &SqlitePool, payload: &mut Value) -> Result<bool, LbError> {
    let Some(object) = payload.as_object_mut() else {
        return Ok(false);
    };
    if object.contains_key("model") {
        return Ok(false);
    }
    let Some(model) = load(pool).await?.default_text_model else {
        return Ok(false);
    };
    object.insert("model".into(), Value::String(model));
    Ok(true)
}

fn incompatible(model: &str) -> LbError {
    CommonError::Validation(format!(
        "configured default model does not support this API: {model}"
    ))
    .into()
}

/// Validate only default-origin requests, retaining the candidates through dispatch.
/// `None` is a supported cloud adapter; `Some` is the local route's candidate set.
pub(crate) async fn default_candidates(
    state: &AppState,
    model: &str,
    route: TextRoute,
    image_input: bool,
) -> Result<Option<Vec<Endpoint>>, LbError> {
    let registered_caps = storage::registered_model_capabilities(state, model).await?;
    if let Some((provider, _)) = parse_cloud_model(model) {
        let caps = registered_caps.unwrap_or_else(|| vec![ModelCapability::TextGeneration]);
        if !caps.contains(&ModelCapability::TextGeneration)
            || image_input && !caps.contains(&ModelCapability::ImageInput)
        {
            return Err(incompatible(model));
        }
        return if matches!(route, TextRoute::Chat)
            || matches!(route, TextRoute::Messages) && provider == "anthropic"
        {
            Ok(None)
        } else {
            Err(incompatible(model))
        };
    }
    parse_quantized_model_name(model)?;
    let has_registered_caps = registered_caps.is_some();
    let mut capabilities = registered_caps.unwrap_or_default();

    let mut required = match route {
        TextRoute::Chat | TextRoute::Messages => vec![SupportedAPI::ChatCompletions],
        TextRoute::Responses => vec![SupportedAPI::Responses],
        TextRoute::LegacyCompletion => Vec::new(),
    };
    if image_input {
        required.push(SupportedAPI::ImageInput);
    }
    let registry = &state.balancer.endpoint_registry;
    let mut known = false;
    let mut compatible = false;
    let mut candidates = Vec::new();
    // Offline metadata proves compatibility without probing a service. Only online
    // candidates are handed to the existing ready/TPS selector.
    for endpoint in registry.list().await {
        let models = registry
            .list_models(endpoint.id)
            .await
            .map_err(|error| LbError::Database(error.to_string()))?;
        let matching: Vec<_> = models
            .iter()
            .filter(|row| endpoint_model_matches(model, row))
            .collect();
        known |= !matching.is_empty();
        if !has_registered_caps {
            for row in &matching {
                for cap in storage::endpoint_capabilities(row) {
                    if !capabilities.contains(&cap) {
                        capabilities.push(cap);
                    }
                }
            }
        }
        // The existing alias helper selects the upstream row in list order.
        // Check that exact row; another alias cannot lend it API capabilities.
        let upstream =
            resolve_runtime_model_name_for_endpoint(model, model, &endpoint.endpoint_type, &models);
        if matching
            .iter()
            .find(|row| row.model_id == upstream)
            .is_some_and(|row| {
                let caps = storage::endpoint_capabilities(row);
                caps.contains(&ModelCapability::TextGeneration)
                    && (!image_input || caps.contains(&ModelCapability::ImageInput))
                    && required.iter().all(|api| row.supported_apis.contains(api))
            })
        {
            compatible = true;
            if endpoint.status == EndpointStatus::Online {
                candidates.push(endpoint);
            }
        }
    }
    if !known && !has_registered_caps {
        return Err(LbError::NotFound(format!(
            "The model '{model}' does not exist"
        )));
    }
    if !capabilities.contains(&ModelCapability::TextGeneration)
        || image_input && !capabilities.contains(&ModelCapability::ImageInput)
    {
        return Err(incompatible(model));
    }
    if known && !compatible {
        return Err(incompatible(model));
    }
    Ok(Some(candidates))
}

/// Responses input images use a different wire content shape from chat.
pub(crate) fn responses_require_image_input(payload: &Value) -> bool {
    payload
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|input| {
            input.iter().any(|message| {
                message
                    .get("content")
                    .and_then(Value::as_array)
                    .is_some_and(|parts| {
                        parts.iter().any(|part| {
                            part.get("type").and_then(Value::as_str) == Some("input_image")
                        })
                    })
            })
        })
}
