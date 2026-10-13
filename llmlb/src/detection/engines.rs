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

pub(super) async fn detect_tgi(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<(EndpointType, String)> {
    // TGI serializes CARGO_PKG_NAME in Info.router.
    detect_native_identity(
        client,
        base_url,
        api_key,
        (
            "/info",
            "router",
            "text-generation-router",
            EndpointType::Tgi,
        ),
    )
    .await
}

pub(super) async fn detect_koboldcpp(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Option<(EndpointType, String)> {
    // KoboldCpp exposes Ollama-compatible tags as well as this native identity.
    detect_native_identity(
        client,
        base_url,
        api_key,
        (
            "/api/extra/version",
            "result",
            "KoboldCpp",
            EndpointType::Koboldcpp,
        ),
    )
    .await
}

async fn detect_native_identity(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
    (path, field, identity, endpoint_type): (&str, &str, &str, EndpointType),
) -> Option<(EndpointType, String)> {
    let mut request = client.get(format!("{base_url}{path}"));
    if let Some(key) = api_key {
        request = request.bearer_auth(key);
    }
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json = response.json::<Value>().await.ok()?;
    (json.get(field).and_then(Value::as_str) == Some(identity)).then(|| {
        (
            endpoint_type,
            format!("{endpoint_type}: {path} {field} is '{identity}'"),
        )
    })
}
