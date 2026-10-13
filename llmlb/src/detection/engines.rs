//! Engine identities verified against their upstream implementations (#887).

use reqwest::Client;
use serde_json::Value;

use super::models_response::ModelsResponse;
use crate::types::endpoint::EndpointType;

pub(super) fn detect_model_owner(models: &ModelsResponse) -> Option<(EndpointType, String)> {
    for owner in models.owners() {
        let (endpoint_type, label) = match owner {
            "sglang" => (EndpointType::Sglang, "SGLang"),
            "lmdeploy" => (EndpointType::Lmdeploy, "LMDeploy"),
            "xinference" => (EndpointType::Xinference, "Xinference"),
            "lemonade" => (EndpointType::Lemonade, "Lemonade"),
            _ => continue,
        };
        return Some((endpoint_type, format!("{label}: owned_by is '{owner}'")));
    }
    None
}

pub(super) async fn detect_native_engine(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<(EndpointType, String)> {
    // TGI serializes CARGO_PKG_NAME in Info.router; KoboldCpp sets result in
    // get_capabilities(). Neither a model name nor generic /props is an identity.
    for (path, field, identity, endpoint_type) in [
        (
            "/info",
            "router",
            "text-generation-router",
            EndpointType::Tgi,
        ),
        (
            "/api/extra/version",
            "result",
            "KoboldCpp",
            EndpointType::Koboldcpp,
        ),
    ] {
        let mut request = client.get(format!("{base_url}{path}"));
        if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }
        let Ok(response) = request.send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(json) = response.json::<Value>().await else {
            continue;
        };
        if json.get(field).and_then(Value::as_str) == Some(identity) {
            return Some((
                endpoint_type,
                format!("{endpoint_type}: {path} {field} is '{identity}'"),
            ));
        }
    }
    None
}
