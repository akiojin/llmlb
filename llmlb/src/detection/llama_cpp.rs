//! llama.cpp identification using the Server header of the shared models probe.
//! The upstream llama-server does not expose /v1/version (#887).

use super::models_response::ModelsResponse;
use reqwest::Client;

/// Detect llama.cpp from its Server response header.
pub async fn detect_llamacpp(client: &Client, base_url: &str) -> Option<String> {
    detect_llamacpp_from_models(&ModelsResponse::fetch(client, base_url, None).await)
}

pub(super) fn detect_llamacpp_from_models(models: &ModelsResponse) -> Option<String> {
    let server = models.server.as_ref()?;
    server
        .contains("llama.cpp")
        .then(|| format!("llama.cpp: Server header contains llama.cpp ({server})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn detect_llamacpp_via_server_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("server", "llama.cpp/0.3.0")
                    .set_body_json(serde_json::json!({
                        "data": [],
                        "object": "list"
                    })),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let result = detect_llamacpp(&client, &server.uri()).await;
        assert!(result.is_some());
        assert!(result.unwrap().contains("llama.cpp"));
    }

    #[tokio::test]
    async fn detect_llamacpp_returns_none_for_non_llamacpp() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [],
                "object": "list"
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "server": "openai",
                "version": "1.0"
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let result = detect_llamacpp(&client, &server.uri()).await;
        assert!(result.is_none());
    }
}
