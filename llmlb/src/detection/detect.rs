//! エンドポイント種別の自動判定（優先順位に従った判定と OpenAI 互換フォールバック）

use super::strata::detect_strata_family;
use super::{
    detect_ollama, detect_xllm,
    engines::{detect_model_owner, detect_native_engine},
    llama_cpp::detect_llamacpp_from_models,
    lm_studio::{detect_lm_studio_from_models, detect_lm_studio_native},
    models_response::ModelsResponse,
    vllm::detect_vllm_from_models,
};
use reqwest::Client;
use tracing::{debug, warn};

use crate::types::endpoint::EndpointType;

/// 検出エラー
#[derive(Debug, Clone)]
pub enum DetectionError {
    /// エンドポイントに接続できない（全プローブが接続エラー）
    Unreachable(String),
    /// 接続はできたが対応タイプに一致しない
    UnsupportedType(String),
}

impl std::fmt::Display for DetectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(msg) => write!(f, "endpoint unreachable: {}", msg),
            Self::UnsupportedType(msg) => write!(f, "unsupported endpoint type: {}", msg),
        }
    }
}

impl std::error::Error for DetectionError {}

/// 検出成功時の結果
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// 判定されたエンドポイントタイプ
    pub endpoint_type: EndpointType,
    /// 判定理由
    pub reason: String,
}

/// Detect endpoint type with a provided HTTP client
pub async fn detect_endpoint_type_with_client(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<DetectionResult, DetectionError> {
    let base_url = base_url.trim_end_matches('/');

    debug!(base_url = %base_url, "Starting endpoint type detection");

    if let Some(reason) = detect_xllm(client, base_url, api_key).await {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::Xllm,
            reason,
        });
    }
    if let Some(reason) = detect_lm_studio_native(client, base_url, api_key).await {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::LmStudio,
            reason,
        });
    }

    // Preserve native-probe priority, but fetch this shared endpoint only once.
    let models = ModelsResponse::fetch(client, base_url, api_key).await;
    if let Some(reason) = detect_lm_studio_from_models(&models) {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::LmStudio,
            reason,
        });
    }
    if let Some(reason) = detect_ollama(client, base_url).await {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::Ollama,
            reason,
        });
    }
    if let Some(reason) = detect_vllm_from_models(&models) {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::Vllm,
            reason,
        });
    }
    // Priority 5: Strata / Project Maya intentionally mimic llama.cpp APIs.
    // Check their public /health markers first to avoid misclassifying them.
    if let Some(result) = detect_strata_family(client, base_url).await {
        debug!(endpoint_type = %result.endpoint_type, "Detected Strata-family endpoint");
        return Ok(result);
    }

    if let Some((endpoint_type, reason)) = detect_model_owner(&models) {
        return Ok(DetectionResult {
            endpoint_type,
            reason,
        });
    }
    // KoboldCpp also implements /props: its explicit identity must win first.
    if let Some((endpoint_type, reason)) = detect_native_engine(client, base_url, api_key).await {
        return Ok(DetectionResult {
            endpoint_type,
            reason,
        });
    }
    if let Some(reason) = detect_llamacpp_from_models(&models) {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::Llamacpp,
            reason,
        });
    }
    if models.is_openai_compatible() {
        return Ok(DetectionResult {
            endpoint_type: EndpointType::OpenaiCompatible,
            reason: "OpenAI-compatible: /v1/models responded 200".to_string(),
        });
    }

    // The shared probe also distinguishes HTTP responses from connection errors;
    // retrying /v1/models here would violate the single-response contract.
    if models.responded {
        warn!(base_url = %base_url, "Endpoint responded but type could not be determined");
        Err(DetectionError::UnsupportedType(format!(
            "endpoint at {} responded but does not match any supported type",
            base_url
        )))
    } else {
        Err(DetectionError::Unreachable(format!(
            "could not connect to {}",
            base_url
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn test_detection_error_display() {
        let unreachable = DetectionError::Unreachable("connection refused".to_string());
        assert!(unreachable.to_string().contains("unreachable"));

        let unsupported = DetectionError::UnsupportedType("no match".to_string());
        assert!(unsupported.to_string().contains("unsupported"));
    }

    #[tokio::test]
    async fn detect_endpoint_type_detects_openai_compatible() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/system"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id": "gpt-test", "object": "model"}
                ]
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let detected = detect_endpoint_type_with_client(&client, &server.uri(), None)
            .await
            .expect("should detect openai-compatible endpoint");
        assert_eq!(
            detected.endpoint_type,
            crate::types::endpoint::EndpointType::OpenaiCompatible
        );
        assert!(detected.reason.contains("/v1/models"));
    }

    #[tokio::test]
    async fn detect_endpoint_type_returns_unsupported_when_response_shape_is_unknown() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/system"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hello": "world"
            })))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let err = detect_endpoint_type_with_client(&client, &server.uri(), None)
            .await
            .expect_err("should fail for unsupported endpoint shape");
        match err {
            DetectionError::UnsupportedType(msg) => {
                assert!(msg.contains("does not match any supported type"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn detect_endpoint_type_returns_unreachable_for_dead_host() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();

        let err = detect_endpoint_type_with_client(&client, "http://127.0.0.1:9", None)
            .await
            .expect_err("connection should fail");
        match err {
            DetectionError::Unreachable(msg) => {
                assert!(msg.contains("could not connect"));
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
