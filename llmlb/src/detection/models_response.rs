//! A single authenticated /v1/models response shared by every detector.

use reqwest::Client;
use serde_json::Value;
use tracing::debug;

pub(super) struct ModelsResponse {
    pub server: Option<String>,
    pub json: Option<Value>,
    pub responded: bool,
}

impl ModelsResponse {
    pub async fn fetch(client: &Client, base_url: &str, api_key: Option<&str>) -> Self {
        let mut request = client.get(format!("{base_url}/v1/models"));
        if let Some(key) = api_key {
            request = request.bearer_auth(key);
        }
        match request.send().await {
            Ok(response) => {
                let server = response
                    .headers()
                    .get("server")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let json = if response.status().is_success() {
                    response.json().await.ok()
                } else {
                    None
                };
                Self {
                    server,
                    json,
                    responded: true,
                }
            }
            Err(error) => {
                debug!(%error, "Shared /v1/models probe failed");
                Self {
                    server: None,
                    json: None,
                    responded: false,
                }
            }
        }
    }

    pub fn owners(&self) -> impl Iterator<Item = &str> {
        self.json
            .as_ref()
            .and_then(|json| json.get("data"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|model| model.get("owned_by").and_then(Value::as_str))
    }

    pub fn is_openai_compatible(&self) -> bool {
        self.json
            .as_ref()
            .is_some_and(|json| json.get("data").is_some() || json.get("object").is_some())
    }
}
