//! モデル名の解析ユーティリティ（量子化サフィックス対応）

use crate::common::error::{CommonError, LbError};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
