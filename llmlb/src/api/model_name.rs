//! モデル名の解析ユーティリティ（量子化サフィックス対応）

use crate::common::error::{CommonError, LbError};
use crate::types::endpoint::{EndpointModel, EndpointType};
use serde_json::Value;

/// 量子化サフィックスを含むモデル名の解析結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModelName {
    /// 元のモデル名（サフィックス含む）
    pub raw: String,
    /// 量子化サフィックスを除いたモデル名
    pub base: String,
    /// 量子化サフィックス（指定がある場合のみ）
    pub quantization: Option<String>,
}

/// `modelname:quantization` 形式を解析する
pub fn parse_quantized_model_name(model: &str) -> Result<ParsedModelName, LbError> {
    let Some(pos) = model.find(':') else {
        return Ok(ParsedModelName {
            raw: model.to_string(),
            base: model.to_string(),
            quantization: None,
        });
    };

    if model[pos + 1..].contains(':') || pos == 0 || pos == model.len() - 1 {
        return Err(LbError::Common(CommonError::Validation(format!(
            "Invalid model name (quantization format): {}",
            model
        ))));
    }

    Ok(ParsedModelName {
        raw: model.to_string(),
        base: model[..pos].to_string(),
        quantization: Some(model[pos + 1..].to_string()),
    })
}

/// Resolve the engine-specific runtime model name for a selected endpoint.
pub fn resolve_runtime_model_name(model: &str, endpoint_type: &EndpointType) -> String {
    crate::models::mapping::resolve_engine_name(model, endpoint_type)
        .map(|resolved| resolved.to_string())
        .unwrap_or_else(|| model.to_string())
}

/// Resolve the runtime model name that the selected endpoint actually advertises.
pub fn resolve_runtime_model_name_for_endpoint(
    requested_model: &str,
    selected_model: &str,
    endpoint_type: &EndpointType,
    endpoint_models: &[EndpointModel],
) -> String {
    if endpoint_models
        .iter()
        .any(|endpoint_model| endpoint_model.model_id == requested_model)
    {
        return requested_model.to_string();
    }

    if let Some(runtime_model) = endpoint_models.iter().find_map(|endpoint_model| {
        if endpoint_model.model_id == selected_model {
            return Some(endpoint_model.model_id.as_str());
        }

        if endpoint_model.canonical_name.as_deref() == Some(selected_model)
            || endpoint_model.canonical_name.as_deref() == Some(requested_model)
        {
            return Some(endpoint_model.model_id.as_str());
        }

        None
    }) {
        return runtime_model.to_string();
    }

    resolve_runtime_model_name(selected_model, endpoint_type)
}

/// Rewrite the request payload's `model` field for the selected endpoint when needed.
pub fn rewrite_payload_model_for_endpoint(
    mut payload: Value,
    selected_model: &str,
    endpoint_type: &EndpointType,
    endpoint_models: &[EndpointModel],
) -> Value {
    let Some(requested_model) = payload.get("model").and_then(Value::as_str) else {
        return payload;
    };

    let runtime_model = resolve_runtime_model_name_for_endpoint(
        requested_model,
        selected_model,
        endpoint_type,
        endpoint_models,
    );
    if runtime_model == requested_model {
        return payload;
    }

    if let Some(object) = payload.as_object_mut() {
        object.insert("model".to_string(), Value::String(runtime_model));
    }

    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::endpoint::SupportedAPI;
    use serde_json::json;
    use uuid::Uuid;

    fn endpoint_model(model_id: &str, canonical_name: Option<&str>) -> EndpointModel {
        EndpointModel {
            endpoint_id: Uuid::nil(),
            model_id: model_id.to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis: vec![SupportedAPI::ChatCompletions],
            canonical_name: canonical_name.map(str::to_string),
        }
    }

    #[test]
    fn no_colon_returns_base_only() {
        let r = parse_quantized_model_name("llama3").unwrap();
        assert_eq!(r.base, "llama3");
        assert_eq!(r.quantization, None);
        assert_eq!(r.raw, "llama3");
    }

    #[test]
    fn colon_splits_base_and_quantization() {
        let r = parse_quantized_model_name("model:Q4_K_M").unwrap();
        assert_eq!(r.base, "model");
        assert_eq!(r.quantization, Some("Q4_K_M".to_string()));
        assert_eq!(r.raw, "model:Q4_K_M");
    }

    #[test]
    fn empty_string_returns_empty_base() {
        let r = parse_quantized_model_name("").unwrap();
        assert_eq!(r.base, "");
        assert_eq!(r.quantization, None);
    }

    #[test]
    fn trailing_colon_is_error() {
        assert!(parse_quantized_model_name("model:").is_err());
    }

    #[test]
    fn leading_colon_is_error() {
        assert!(parse_quantized_model_name(":quant").is_err());
    }

    #[test]
    fn multiple_colons_is_error() {
        assert!(parse_quantized_model_name("model:q1:q2").is_err());
    }

    #[test]
    fn long_model_name() {
        let long = "a".repeat(500);
        let r = parse_quantized_model_name(&long).unwrap();
        assert_eq!(r.base, long);
        assert_eq!(r.quantization, None);
    }

    #[test]
    fn unicode_model_name() {
        let r = parse_quantized_model_name("モデル:量子化").unwrap();
        assert_eq!(r.base, "モデル");
        assert_eq!(r.quantization, Some("量子化".to_string()));
    }

    #[test]
    fn spaces_in_model_name() {
        let r = parse_quantized_model_name("my model:Q8_0").unwrap();
        assert_eq!(r.base, "my model");
        assert_eq!(r.quantization, Some("Q8_0".to_string()));
    }

    #[test]
    fn special_characters_in_model_name() {
        let r = parse_quantized_model_name("org/model-v2.1:Q4_K_S").unwrap();
        assert_eq!(r.base, "org/model-v2.1");
        assert_eq!(r.quantization, Some("Q4_K_S".to_string()));
    }

    #[test]
    fn mixed_case() {
        let r = parse_quantized_model_name("MyModel:q4_k_m").unwrap();
        assert_eq!(r.base, "MyModel");
        assert_eq!(r.quantization, Some("q4_k_m".to_string()));
    }

    #[test]
    fn parsed_model_name_partial_eq() {
        let a = ParsedModelName {
            raw: "m:Q4".to_string(),
            base: "m".to_string(),
            quantization: Some("Q4".to_string()),
        };
        let b = a.clone();
        assert_eq!(a, b);

        let c = ParsedModelName {
            raw: "m".to_string(),
            base: "m".to_string(),
            quantization: None,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn resolve_runtime_model_name_uses_engine_alias_for_canonical_input() {
        let resolved = resolve_runtime_model_name("openai/gpt-oss-20b", &EndpointType::Ollama);
        assert_eq!(resolved, "gpt-oss:20b");
    }

    #[test]
    fn rewrite_payload_model_for_endpoint_replaces_canonical_model() {
        let payload = json!({
            "model": "openai/gpt-oss-20b",
            "messages": [{"role": "user", "content": "hello"}]
        });

        let endpoint_models = vec![endpoint_model("gpt-oss:20b", Some("openai/gpt-oss-20b"))];
        let rewritten = rewrite_payload_model_for_endpoint(
            payload,
            "openai/gpt-oss-20b",
            &EndpointType::Ollama,
            &endpoint_models,
        );
        assert_eq!(rewritten["model"], "gpt-oss:20b");
    }

    #[test]
    fn rewrite_payload_model_for_endpoint_leaves_alias_input_unchanged() {
        let payload = json!({
            "model": "gpt-oss:20b",
            "messages": [{"role": "user", "content": "hello"}]
        });

        let endpoint_models = vec![endpoint_model("gpt-oss:20b", Some("openai/gpt-oss-20b"))];
        let rewritten = rewrite_payload_model_for_endpoint(
            payload.clone(),
            "openai/gpt-oss-20b",
            &EndpointType::Ollama,
            &endpoint_models,
        );
        assert_eq!(rewritten, payload);
    }

    #[test]
    fn rewrite_payload_model_for_endpoint_prefers_selected_endpoint_alias() {
        let payload = json!({
            "model": "Qwen/Qwen3.5-35B-A3B",
            "messages": [{"role": "user", "content": "hello"}]
        });
        let endpoint_models = vec![endpoint_model(
            "qwen/qwen3.5-35b-a3b:2",
            Some("Qwen/Qwen3.5-35B-A3B"),
        )];

        let rewritten = rewrite_payload_model_for_endpoint(
            payload,
            "Qwen/Qwen3.5-35B-A3B",
            &EndpointType::LmStudio,
            &endpoint_models,
        );

        assert_eq!(rewritten["model"], "qwen/qwen3.5-35b-a3b:2");
    }

    #[test]
    fn rewrite_payload_model_for_endpoint_keeps_requested_alias_if_endpoint_has_it() {
        let payload = json!({
            "model": "qwen3.5-35b-a3b",
            "messages": [{"role": "user", "content": "hello"}]
        });
        let endpoint_models = vec![endpoint_model(
            "qwen3.5-35b-a3b",
            Some("Qwen/Qwen3.5-35B-A3B"),
        )];

        let rewritten = rewrite_payload_model_for_endpoint(
            payload.clone(),
            "Qwen/Qwen3.5-35B-A3B",
            &EndpointType::LmStudio,
            &endpoint_models,
        );

        assert_eq!(rewritten, payload);
    }
}
