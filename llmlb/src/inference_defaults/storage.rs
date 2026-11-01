//! Versioned inference configuration persisted in the existing settings table.

use crate::common::error::{CommonError, LbError};
use crate::db::settings::SettingsStorage;
use crate::types::endpoint::{EndpointModel, SupportedAPI};
use crate::types::ModelCapability;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

/// Reserved settings key owned by the typed management API.
pub const SETTINGS_KEY: &str = "inference.defaults";

/// Public configuration shared by management and inference requests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct InferenceDefaults {
    /// Public text model ID, or `None` to require an explicit model.
    pub default_text_model: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredDefaults {
    version: u32,
    default_text_model: Value,
}

fn validation(message: &str) -> LbError {
    LbError::Common(CommonError::Validation(message.to_string()))
}

fn validate_string(model: &str) -> Result<(), LbError> {
    if model.is_empty() || model.trim() != model || model.chars().any(char::is_control) {
        return Err(validation(
            "default_text_model must be a nonempty model ID without surrounding whitespace or control characters",
        ));
    }
    Ok(())
}

impl InferenceDefaults {
    /// Parse the strict wire shape, preserving the difference between missing and null.
    pub fn from_value(value: Value) -> Result<Self, LbError> {
        let object = value
            .as_object()
            .ok_or_else(|| validation("expected an object containing only default_text_model"))?;
        if object.len() != 1 || !object.contains_key("default_text_model") {
            return Err(validation(
                "default_text_model is required and unknown fields are not allowed",
            ));
        }
        let default_text_model = match &object["default_text_model"] {
            Value::Null => None,
            Value::String(model) => {
                validate_string(model)?;
                Some(model.clone())
            }
            _ => return Err(validation("default_text_model must be a string or null")),
        };
        Ok(Self { default_text_model })
    }
}

/// Read one immutable request snapshot. Corruption is an error rather than an unset value.
pub async fn load(pool: &SqlitePool) -> Result<InferenceDefaults, LbError> {
    let Some(raw) = SettingsStorage::new(pool.clone())
        .get_setting(SETTINGS_KEY)
        .await?
    else {
        return Ok(InferenceDefaults::default());
    };
    let invalid = || LbError::Internal("Stored inference defaults are invalid".to_string());
    let stored: StoredDefaults = serde_json::from_str(&raw).map_err(|_| invalid())?;
    if stored.version != 1 {
        return Err(invalid());
    }
    InferenceDefaults::from_value(
        serde_json::json!({"default_text_model":stored.default_text_model}),
    )
    .map_err(|_| invalid())
}

/// Validate before writing; the single SQLite statement commits the whole versioned value.
pub async fn save(state: &AppState, defaults: &InferenceDefaults) -> Result<(), LbError> {
    if let Some(model) = defaults.default_text_model.as_deref() {
        validate_model(state, model).await?;
    }
    let stored = StoredDefaults {
        version: 1,
        default_text_model: serde_json::to_value(&defaults.default_text_model)
            .map_err(|_| LbError::Internal("Could not encode inference defaults".to_string()))?,
    };
    let raw = serde_json::to_string(&stored)
        .map_err(|_| LbError::Internal("Could not encode inference defaults".to_string()))?;
    SettingsStorage::new(state.db_pool.clone())
        .set_setting(SETTINGS_KEY, &raw)
        .await
}

/// Validate a saved model without network probes, downloads, or availability requirements.
pub async fn validate_model(state: &AppState, model: &str) -> Result<(), LbError> {
    validate_string(model)?;
    if let Some((provider, model_name)) = crate::api::openai::parse_cloud_model(model) {
        let env_key = match provider.as_str() {
            "openai" => "OPENAI_API_KEY",
            "google" => "GOOGLE_API_KEY",
            "anthropic" => "ANTHROPIC_API_KEY",
            _ => return Err(validation("unsupported cloud provider")),
        };
        if !std::env::var(env_key).is_ok_and(|key| !key.trim().is_empty()) {
            return Err(validation(
                "configured default model requires a configured cloud provider",
            ));
        }
        if let Some(caps) = local_model_capabilities(state, model).await? {
            return if caps.contains(&ModelCapability::TextGeneration) {
                Ok(())
            } else {
                Err(validation(
                    "default_text_model must support text generation",
                ))
            };
        }
        // Existing cloud catalog rows infer text generation from their chat adapter.
        // Read only the identity snapshot: saving must never refresh the catalog.
        let known = crate::api::cloud_models::cached_models_snapshot()
            .await
            .into_iter()
            .any(|cached| {
                crate::api::openai::parse_cloud_model(&cached.id).is_some_and(
                    |(cached_provider, cached_name)| {
                        cached_provider == provider && cached_name == model_name
                    },
                )
            });
        return if known {
            Ok(())
        } else {
            Err(validation("default_text_model must identify a known model"))
        };
    }
    crate::api::model_name::parse_quantized_model_name(model)?;
    match local_model_capabilities(state, model).await? {
        Some(caps) if caps.contains(&ModelCapability::TextGeneration) => Ok(()),
        Some(_) => Err(validation(
            "default_text_model must support text generation",
        )),
        None => Err(validation("default_text_model must identify a known model")),
    }
}

fn equivalent(left: &str, right: &str) -> bool {
    left == right
        || crate::api::openai::parse_cloud_model(left)
            .is_some_and(|parsed| Some(parsed) == crate::api::openai::parse_cloud_model(right))
        || match (
            crate::models::mapping::find_mapping(left),
            crate::models::mapping::find_mapping(right),
        ) {
            (Some(left), Some(right)) => left.canonical == right.canonical,
            _ => false,
        }
}

fn api_capability(api: SupportedAPI) -> ModelCapability {
    match api {
        SupportedAPI::ChatCompletions | SupportedAPI::Responses => ModelCapability::TextGeneration,
        SupportedAPI::Embeddings => ModelCapability::Embedding,
        SupportedAPI::ImageInput => ModelCapability::ImageInput,
        SupportedAPI::ImageGeneration => ModelCapability::ImageGeneration,
    }
}

pub(crate) fn endpoint_capabilities(model: &EndpointModel) -> Vec<ModelCapability> {
    let mut caps: Vec<ModelCapability> = model
        .capabilities
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|name| {
            serde_json::from_value::<ModelCapability>(Value::String(name.clone()))
                .ok()
                .or_else(|| SupportedAPI::from_api_str(name).map(api_capability))
                .or_else(|| {
                    matches!(name.as_str(), "completion" | "text_completion" | "generate")
                        .then_some(ModelCapability::TextGeneration)
                })
        })
        .collect();
    // Explicit modality metadata takes precedence over generic/default API inference.
    if caps.is_empty() {
        caps = model
            .supported_apis
            .iter()
            .copied()
            .map(api_capability)
            .collect();
        if caps.is_empty() {
            caps.push(ModelCapability::TextGeneration);
        }
    }
    caps
}

/// Inspect known local capability metadata, including offline synced endpoint models.
/// A registered model's explicit modality metadata takes precedence over endpoint inference.
pub async fn local_model_capabilities(
    state: &AppState,
    model: &str,
) -> Result<Option<Vec<ModelCapability>>, LbError> {
    if let Some(caps) = registered_model_capabilities(state, model).await? {
        return Ok(Some(caps));
    }
    endpoint_model_capabilities(state, model).await
}

pub(crate) async fn registered_model_capabilities(
    state: &AppState,
    model: &str,
) -> Result<Option<Vec<ModelCapability>>, LbError> {
    for registered in crate::api::models::list_registered_models(&state.db_pool).await? {
        if equivalent(model, &registered.name) {
            return Ok(Some(registered.get_capabilities()));
        }
    }
    Ok(None)
}

async fn endpoint_model_capabilities(
    state: &AppState,
    model: &str,
) -> Result<Option<Vec<ModelCapability>>, LbError> {
    let registry = &state.balancer.endpoint_registry;
    let mut found = false;
    let mut caps = Vec::new();
    for endpoint in registry.list().await {
        for endpoint_model in registry
            .list_models(endpoint.id)
            .await
            .map_err(|error| LbError::Database(error.to_string()))?
        {
            if crate::registry::endpoints::endpoint_model_matches(model, &endpoint_model) {
                found = true;
                for cap in endpoint_capabilities(&endpoint_model) {
                    if !caps.contains(&cap) {
                        caps.push(cap);
                    }
                }
            }
        }
    }
    Ok(found.then_some(caps))
}
