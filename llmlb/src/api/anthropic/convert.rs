//! Anthropic Messages リクエスト/レスポンスと OpenAI 互換形式の相互変換

use crate::token::TokenUsage;
use axum::http::StatusCode;
use axum::response::Response;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::anthropic_error_response;

#[derive(Debug)]
pub(super) struct ConvertedAnthropicRequest {
    pub(super) openai_payload: Value,
    pub(super) request_text: String,
    pub(super) stream: bool,
}

/// Anthropic の tool_result.content（文字列または content ブロック配列）を
/// OpenAI tool メッセージ用の文字列へ変換する。配列形式（Claude Code が常用）でも
/// 中身を欠落させない。
pub(super) fn anthropic_tool_result_content_to_string(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => {
            let mut parts = Vec::new();
            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = item.get("text").and_then(Value::as_str) {
                            parts.push(t.to_string());
                        }
                    }
                    // text 以外（image 等）は JSON で温存して情報欠落を防ぐ
                    _ => parts.push(item.to_string()),
                }
            }
            parts.join("\n")
        }
        Some(v) if !v.is_null() => v.to_string(),
        _ => String::new(),
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn anthropic_request_to_openai(
    payload: &Value,
) -> Result<ConvertedAnthropicRequest, Response> {
    let model = extract_model(payload)?;
    let max_tokens = payload
        .get("max_tokens")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "max_tokens is required",
            )
        })?;
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let message_values = payload
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "messages must be an array",
            )
        })?;

    let mut request_text_parts = Vec::new();
    let mut openai_messages = Vec::new();

    if let Some(system) = payload.get("system") {
        let system_text = flatten_anthropic_text_content(system, "system")?;
        if !system_text.is_empty() {
            openai_messages.push(json!({
                "role": "system",
                "content": system_text
            }));
            request_text_parts.push(format!("system: {}", system_text));
        }
    }

    for (index, message) in message_values.iter().enumerate() {
        let role = message.get("role").and_then(Value::as_str).ok_or_else(|| {
            anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                format!("messages[{}].role is required", index),
            )
        })?;
        if !matches!(role, "user" | "assistant") {
            return Err(anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                format!("messages[{}].role must be 'user' or 'assistant'", index),
            ));
        }

        let content = message.get("content").ok_or_else(|| {
            anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                format!("messages[{}].content is required", index),
            )
        })?;

        // assistant メッセージ内の tool_use ブロックを OpenAI の assistant + tool_calls に変換する。
        // 併記された text も欠落させない（マルチターンのツール会話の整合性に必要）。
        if role == "assistant" {
            if let Some(content_array) = content.as_array() {
                let has_tool_use = content_array
                    .iter()
                    .any(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"));

                if has_tool_use {
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();
                    for item in content_array {
                        match item.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(t) = item.get("text").and_then(Value::as_str) {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("tool_use") => {
                                let id = item.get("id").and_then(Value::as_str).unwrap_or("");
                                let name =
                                    item.get("name").and_then(Value::as_str).unwrap_or_default();
                                // OpenAI は arguments を JSON 文字列で要求する
                                let arguments = item
                                    .get("input")
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "{}".to_string());
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": arguments
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let joined_text = text_parts.join("");
                    let mut assistant_msg = Map::new();
                    assistant_msg.insert("role".to_string(), json!("assistant"));
                    assistant_msg.insert(
                        "content".to_string(),
                        if joined_text.is_empty() {
                            Value::Null
                        } else {
                            json!(joined_text)
                        },
                    );
                    assistant_msg.insert("tool_calls".to_string(), Value::Array(tool_calls));
                    openai_messages.push(Value::Object(assistant_msg));
                    request_text_parts.push(format!("assistant(tool_use): {}", joined_text));
                    continue;
                }
            }
        }

        // Handle tool_result content blocks in user messages
        if role == "user" {
            if let Some(content_array) = content.as_array() {
                let has_tool_result = content_array
                    .iter()
                    .any(|item| item.get("type").and_then(Value::as_str) == Some("tool_result"));

                if has_tool_result {
                    // Anthropic tool_result を OpenAI tool メッセージへ変換する。
                    // 併記された text ブロックは破棄せず、後続の user メッセージとして保持する。
                    let mut user_text_parts = Vec::new();
                    for content_item in content_array {
                        match content_item.get("type").and_then(Value::as_str) {
                            Some("tool_result") => {
                                let tool_use_id = content_item
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown");
                                let result_content = anthropic_tool_result_content_to_string(
                                    content_item.get("content"),
                                );
                                openai_messages.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": result_content
                                }));
                                request_text_parts.push(format!(
                                    "tool_result[{}]: {}",
                                    tool_use_id, result_content
                                ));
                            }
                            Some("text") => {
                                if let Some(t) = content_item.get("text").and_then(Value::as_str) {
                                    user_text_parts.push(t.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    if !user_text_parts.is_empty() {
                        let joined = user_text_parts.join("\n");
                        openai_messages.push(json!({ "role": "user", "content": joined }));
                        request_text_parts.push(format!("user: {}", joined));
                    }
                    continue; // tool_result メッセージは通常のテキスト処理をスキップ
                }
            }
        }

        let text =
            flatten_anthropic_text_content(content, &format!("messages[{}].content", index))?;
        openai_messages.push(json!({
            "role": role,
            "content": text
        }));
        request_text_parts.push(format!("{}: {}", role, text));
    }

    let mut body = Map::new();
    body.insert("model".to_string(), Value::String(model));
    body.insert("messages".to_string(), Value::Array(openai_messages));
    body.insert("max_tokens".to_string(), Value::Number(max_tokens.into()));
    body.insert("stream".to_string(), Value::Bool(stream));

    if let Some(temperature) = payload.get("temperature").and_then(Value::as_f64) {
        body.insert("temperature".to_string(), json!(temperature));
    }
    if let Some(top_p) = payload.get("top_p").and_then(Value::as_f64) {
        body.insert("top_p".to_string(), json!(top_p));
    }
    // top_k / metadata は OpenAI 互換側でも同名で受理されるため、値を解釈せずそのまま転送する。
    for key in ["top_k", "metadata"] {
        if let Some(value) = payload.get(key).filter(|value| !value.is_null()) {
            body.insert(key.to_string(), value.clone());
        }
    }
    if let Some(stop_sequences) = payload.get("stop_sequences") {
        body.insert(
            "stop".to_string(),
            normalize_stop_sequences(stop_sequences)?,
        );
    }

    // Convert Anthropic tools to OpenAI functions format
    if let Some(anthropic_tools) = payload.get("tools").and_then(Value::as_array) {
        let openai_tools: Result<Vec<_>, _> = anthropic_tools
            .iter()
            .map(convert_anthropic_tool_to_openai)
            .collect();
        body.insert("tools".to_string(), Value::Array(openai_tools?));
    }

    // Convert Anthropic tool_choice to OpenAI format
    if let Some(anthropic_tool_choice) = payload.get("tool_choice") {
        body.insert(
            "tool_choice".to_string(),
            convert_anthropic_tool_choice_to_openai(anthropic_tool_choice)?,
        );
    }

    Ok(ConvertedAnthropicRequest {
        openai_payload: Value::Object(body),
        request_text: request_text_parts.join("\n"),
        stream,
    })
}

#[allow(clippy::result_large_err)]
pub(super) fn convert_anthropic_tool_to_openai(tool: &Value) -> Result<Value, Response> {
    let name = tool.get("name").and_then(Value::as_str).ok_or_else(|| {
        anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "tool.name is required",
        )
    })?;
    let description = tool
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("");
    let input_schema = tool.get("input_schema").ok_or_else(|| {
        anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "tool.input_schema is required",
        )
    })?;

    // Convert input_schema (Anthropic format) to parameters (OpenAI format)
    let mut parameters = Map::new();
    if let Some(schema_type) = input_schema.get("type") {
        parameters.insert("type".to_string(), schema_type.clone());
    }
    if let Some(properties) = input_schema.get("properties") {
        parameters.insert("properties".to_string(), properties.clone());
    }
    if let Some(required) = input_schema.get("required") {
        parameters.insert("required".to_string(), required.clone());
    }

    Ok(json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": parameters
        }
    }))
}

#[allow(clippy::result_large_err)]
pub(super) fn convert_anthropic_tool_choice_to_openai(
    tool_choice: &Value,
) -> Result<Value, Response> {
    if let Some(tool_choice_type) = tool_choice.get("type").and_then(Value::as_str) {
        match tool_choice_type {
            "auto" => Ok(Value::String("auto".to_string())),
            "any" => Ok(Value::String("required".to_string())),
            "tool" => {
                let name = tool_choice
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anthropic_error_response(
                            StatusCode::BAD_REQUEST,
                            "invalid_request_error",
                            "tool_choice.name is required when type is 'tool'",
                        )
                    })?;
                Ok(json!({
                    "type": "function",
                    "function": {
                        "name": name
                    }
                }))
            }
            _ => Err(anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                format!("unknown tool_choice type: {}", tool_choice_type),
            )),
        }
    } else {
        Err(anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "tool_choice.type is required",
        ))
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn normalize_stop_sequences(value: &Value) -> Result<Value, Response> {
    let sequences = value.as_array().ok_or_else(|| {
        anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "stop_sequences must be an array of strings",
        )
    })?;
    let mut normalized = Vec::with_capacity(sequences.len());
    for item in sequences {
        let Some(sequence) = item.as_str() else {
            return Err(anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "stop_sequences must be an array of strings",
            ));
        };
        normalized.push(Value::String(sequence.to_string()));
    }
    Ok(Value::Array(normalized))
}

#[allow(clippy::result_large_err)]
pub(super) fn flatten_anthropic_text_content(
    value: &Value,
    field_name: &str,
) -> Result<String, Response> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                    return Err(anthropic_error_response(
                        StatusCode::BAD_REQUEST,
                        "invalid_request_error",
                        format!("{} content blocks must have a type", field_name),
                    ));
                };
                if item_type != "text" {
                    return Err(anthropic_error_response(
                        StatusCode::BAD_REQUEST,
                        "invalid_request_error",
                        format!(
                            "{} content block type '{}' is not supported",
                            field_name, item_type
                        ),
                    ));
                }
                let Some(block_text) = item.get("text").and_then(Value::as_str) else {
                    return Err(anthropic_error_response(
                        StatusCode::BAD_REQUEST,
                        "invalid_request_error",
                        format!("{} text content blocks must include text", field_name),
                    ));
                };
                text.push_str(block_text);
            }
            Ok(text)
        }
        _ => Err(anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            format!("{} must be a string or text content array", field_name),
        )),
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn extract_model(payload: &Value) -> Result<String, Response> {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            anthropic_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                "model is required",
            )
        })?;
    if model.trim().is_empty() {
        return Err(anthropic_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            "model must not be empty",
        ));
    }
    Ok(model.to_string())
}

/// Convert OpenAI tool_call format to Anthropic tool_use content block format
pub(super) fn convert_openai_tool_call_to_anthropic_tool_use(tool_call: &Value) -> Option<Value> {
    let func = tool_call.get("function")?;
    let tool_name = func.get("name").and_then(Value::as_str)?;
    let tool_id = tool_call.get("id").and_then(Value::as_str)?;
    let arguments_str = func
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");

    // Parse arguments JSON - if parsing fails, use empty object as fallback
    let input = serde_json::from_str(arguments_str).unwrap_or_else(|_| Value::Object(Map::new()));

    Some(json!({
        "type": "tool_use",
        "id": tool_id,
        "name": tool_name,
        "input": input
    }))
}

pub(super) fn openai_to_anthropic_message_response(
    body: &Value,
    model: &str,
    usage: &TokenUsage,
) -> Value {
    let choice = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());

    let finish_reason = choice
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str);

    let message = choice.and_then(|choice| choice.get("message"));

    let mut content = Vec::new();

    // Add text content if present
    let text = extract_openai_response_text(body);
    if !text.is_empty() {
        content.push(json!({
            "type": "text",
            "text": text
        }));
    }

    // Add tool_use blocks if tool_calls are present
    if let Some(tool_calls) = message
        .and_then(|msg| msg.get("tool_calls"))
        .and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            if let Some(tool_use_block) = convert_openai_tool_call_to_anthropic_tool_use(tool_call)
            {
                content.push(tool_use_block);
            }
        }
    }

    // If no content was added, add an empty text block
    if content.is_empty() {
        content.push(json!({
            "type": "text",
            "text": ""
        }));
    }

    let stop_reason = if finish_reason == Some("tool_calls") {
        "tool_use"
    } else {
        finish_reason
            .map(map_finish_reason_to_stop_reason)
            .unwrap_or("end_turn")
    };

    json!({
        "id": body
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("msg_{}", Uuid::new_v4().simple())),
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": content,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": usage.input_tokens.unwrap_or(0),
            "output_tokens": usage.output_tokens.unwrap_or(0)
        }
    })
}

pub(super) fn extract_openai_response_text(body: &Value) -> String {
    body.get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    choice
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        })
        .unwrap_or_default()
}

pub(super) fn map_finish_reason_to_stop_reason(finish_reason: &str) -> &'static str {
    match finish_reason {
        "length" => "max_tokens",
        "stop" => "end_turn",
        "tool_calls" => "tool_use",
        _ => "end_turn",
    }
}
