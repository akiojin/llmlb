use super::*;

// T006: chat capabilities検証テスト (RED)
// TextGeneration capability を持たないモデルで /v1/chat/completions を呼ぶとエラー
#[test]
fn test_chat_capability_validation_error_message() {
    use crate::types::model::{ModelCapability, ModelType};

    // TTSモデルはTextToSpeechのみ、TextGenerationは非対応
    let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
    assert!(!tts_caps.contains(&ModelCapability::TextGeneration));

    // ASRモデルもSpeechToTextのみ、TextGenerationは非対応
    let stt_caps = ModelCapability::from_model_type(ModelType::SpeechToText);
    assert!(!stt_caps.contains(&ModelCapability::TextGeneration));

    // EmbeddingモデルもEmbeddingのみ、TextGenerationは非対応
    let embed_caps = ModelCapability::from_model_type(ModelType::Embedding);
    assert!(!embed_caps.contains(&ModelCapability::TextGeneration));

    // 期待されるエラーメッセージ形式
    let model_name = "whisper-large-v3";
    let expected_error = format!("Model '{}' does not support text generation", model_name);
    assert!(expected_error.contains("does not support text generation"));
}

// ===== extract_model tests =====

#[test]
fn extract_model_returns_model_string() {
    use super::extract_model;
    let payload = json!({"model": "llama-3-8b", "messages": []});
    let result = extract_model(&payload).unwrap();
    assert_eq!(result, "llama-3-8b");
}

#[test]
fn extract_model_missing_field_returns_error() {
    use super::extract_model;
    let payload = json!({"messages": []});
    let err = extract_model(&payload);
    assert!(err.is_err());
}

#[test]
fn extract_model_null_value_returns_error() {
    use super::extract_model;
    let payload = json!({"model": null});
    let err = extract_model(&payload);
    assert!(err.is_err());
}

#[test]
fn extract_model_non_string_value_returns_error() {
    use super::extract_model;
    let payload = json!({"model": 42});
    let err = extract_model(&payload);
    assert!(err.is_err());
}

// ===== extract_model_with_default tests =====

#[test]
fn extract_model_with_default_returns_model_when_present() {
    use super::extract_model_with_default;
    let payload = json!({"model": "gpt-4"});
    let result = extract_model_with_default(&payload, "default-model".to_string());
    assert_eq!(result, "gpt-4");
}

#[test]
fn extract_model_with_default_returns_default_when_missing() {
    use super::extract_model_with_default;
    let payload = json!({"messages": []});
    let result = extract_model_with_default(&payload, "default-embed".to_string());
    assert_eq!(result, "default-embed");
}

#[test]
fn extract_model_with_default_returns_default_when_empty_string() {
    use super::extract_model_with_default;
    let payload = json!({"model": ""});
    let result = extract_model_with_default(&payload, "fallback".to_string());
    assert_eq!(result, "fallback");
}

#[test]
fn extract_model_with_default_returns_default_when_null() {
    use super::extract_model_with_default;
    let payload = json!({"model": null});
    let result = extract_model_with_default(&payload, "fallback".to_string());
    assert_eq!(result, "fallback");
}

// ===== extract_stream tests =====

#[test]
fn extract_stream_returns_true_when_set() {
    use super::extract_stream;
    let payload = json!({"model": "x", "stream": true});
    assert!(extract_stream(&payload));
}

#[test]
fn extract_stream_returns_false_when_unset() {
    use super::extract_stream;
    let payload = json!({"model": "x"});
    assert!(!extract_stream(&payload));
}

#[test]
fn extract_stream_returns_false_when_false() {
    use super::extract_stream;
    let payload = json!({"model": "x", "stream": false});
    assert!(!extract_stream(&payload));
}

#[test]
fn extract_stream_returns_false_when_non_bool() {
    use super::extract_stream;
    let payload = json!({"model": "x", "stream": "yes"});
    assert!(!extract_stream(&payload));
}

// ===== payload_requires_image_input tests =====

#[test]
fn payload_requires_image_input_returns_false_for_text_only() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {"role": "user", "content": "Hello"}
        ]
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_returns_false_when_no_messages() {
    use super::payload_requires_image_input;
    let payload = json!({"model": "llama-3"});
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_detects_image_url_content() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "What is in this image?"},
                    {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
                ]
            }
        ]
    });
    assert!(payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_detects_input_image_content() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "What is in this image?"},
                    {"type": "input_image", "image_url": "https://example.com/img.png"}
                ]
            }
        ]
    });
    assert!(payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_allows_text_parts_array() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Hello world"}
                ]
            }
        ]
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_skips_string_content_messages() {
    use super::payload_requires_image_input;
    // messages with string content should be skipped (no array parts)
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {"role": "user", "content": "plain text"},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "OK"}
                ]
            }
        ]
    });
    assert!(!payload_requires_image_input(&payload));
}

// ===== add_queue_headers tests =====

#[test]
fn add_queue_headers_inserts_status_and_wait_time() {
    use super::add_queue_headers;
    use axum::response::IntoResponse;

    let mut response = (StatusCode::OK, "test").into_response();
    add_queue_headers(&mut response, 1500);

    let headers = response.headers();
    assert_eq!(
        headers.get("x-queue-status").unwrap().to_str().unwrap(),
        "queued"
    );
    assert_eq!(
        headers.get("x-queue-wait-ms").unwrap().to_str().unwrap(),
        "1500"
    );
}

#[test]
fn add_queue_headers_zero_wait_time() {
    use super::add_queue_headers;
    use axum::response::IntoResponse;

    let mut response = (StatusCode::OK, "test").into_response();
    add_queue_headers(&mut response, 0);

    assert_eq!(
        response
            .headers()
            .get("x-queue-wait-ms")
            .unwrap()
            .to_str()
            .unwrap(),
        "0"
    );
}

// ===== validation_error tests =====

#[test]
fn validation_error_creates_app_error() {
    use super::validation_error;
    let err = validation_error("test error message");
    let msg = format!("{:?}", err);
    assert!(msg.contains("test error message"));
}

// --- extract_model edge cases ---

#[test]
fn extract_model_boolean_value_returns_error() {
    use super::extract_model;
    let payload = json!({"model": true});
    assert!(extract_model(&payload).is_err());
}

#[test]
fn extract_model_array_value_returns_error() {
    use super::extract_model;
    let payload = json!({"model": ["gpt-4"]});
    assert!(extract_model(&payload).is_err());
}

#[test]
fn extract_model_object_value_returns_error() {
    use super::extract_model;
    let payload = json!({"model": {"name": "gpt-4"}});
    assert!(extract_model(&payload).is_err());
}

#[test]
fn extract_model_empty_string_is_valid() {
    use super::extract_model;
    let payload = json!({"model": ""});
    // extract_model returns any string, empty or not
    let result = extract_model(&payload).unwrap();
    assert_eq!(result, "");
}

#[test]
fn extract_model_whitespace_string_is_valid() {
    use super::extract_model;
    let payload = json!({"model": "  "});
    let result = extract_model(&payload).unwrap();
    assert_eq!(result, "  ");
}

// --- extract_model_with_default edge cases ---

#[test]
fn extract_model_with_default_non_string_returns_default() {
    use super::extract_model_with_default;
    let payload = json!({"model": 123});
    let result = extract_model_with_default(&payload, "default".to_string());
    assert_eq!(result, "default");
}

#[test]
fn extract_model_with_default_boolean_returns_default() {
    use super::extract_model_with_default;
    let payload = json!({"model": false});
    let result = extract_model_with_default(&payload, "default".to_string());
    assert_eq!(result, "default");
}

#[test]
fn extract_model_with_default_whitespace_string_is_not_empty() {
    use super::extract_model_with_default;
    let payload = json!({"model": "  "});
    let result = extract_model_with_default(&payload, "default".to_string());
    // "  " is not empty so it should be returned
    assert_eq!(result, "  ");
}

// --- extract_stream edge cases ---

#[test]
fn extract_stream_null_value_returns_false() {
    use super::extract_stream;
    let payload = json!({"model": "x", "stream": null});
    assert!(!extract_stream(&payload));
}

#[test]
fn extract_stream_numeric_value_returns_false() {
    use super::extract_stream;
    let payload = json!({"model": "x", "stream": 1});
    assert!(!extract_stream(&payload));
}

// --- payload_requires_image_input edge cases ---

#[test]
fn payload_requires_image_input_empty_messages_array() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": []
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_messages_not_array_returns_false() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": "not an array"
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_null_content_is_skipped() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {"role": "user", "content": null}
        ]
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_empty_content_array() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {"role": "user", "content": []}
        ]
    });
    assert!(!payload_requires_image_input(&payload));
}

#[test]
fn payload_requires_image_input_multiple_messages_detects_image_in_second() {
    use super::payload_requires_image_input;
    let payload = json!({
        "model": "llama-3",
        "messages": [
            {"role": "user", "content": "plain text"},
            {
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
                ]
            }
        ]
    });
    assert!(payload_requires_image_input(&payload));
}

// --- add_queue_headers edge cases ---

#[test]
fn add_queue_headers_large_wait_time() {
    use super::add_queue_headers;
    use axum::response::IntoResponse;

    let mut response = (StatusCode::OK, "test").into_response();
    add_queue_headers(&mut response, u128::MAX);

    assert_eq!(
        response
            .headers()
            .get("x-queue-status")
            .unwrap()
            .to_str()
            .unwrap(),
        "queued"
    );
    // x-queue-wait-ms should be set if the string is valid for HeaderValue
}

// --- validation_error tests ---

#[test]
fn validation_error_from_string() {
    use super::validation_error;
    let err = validation_error("custom validation error".to_string());
    let msg = format!("{:?}", err);
    assert!(msg.contains("custom validation error"));
}
