//! モデル名の解析ユーティリティ（量子化サフィックス対応）

use llm_router_common::error::{CommonError, RouterError};

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
pub fn parse_quantized_model_name(model: &str) -> Result<ParsedModelName, RouterError> {
    let Some(pos) = model.find(':') else {
        return Ok(ParsedModelName {
            raw: model.to_string(),
            base: model.to_string(),
            quantization: None,
        });
    };

    if model[pos + 1..].contains(':') || pos == 0 || pos == model.len() - 1 {
        return Err(RouterError::Common(CommonError::Validation(format!(
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
