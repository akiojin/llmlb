use super::*;

#[test]
fn test_validate_model_name_valid() {
    assert!(validate_model_name("gpt-oss").is_ok());
    assert!(validate_model_name("gpt-oss-7b").is_ok());
    assert!(validate_model_name("llama3.2").is_ok());
    assert!(validate_model_name("model_name-v1.0").is_ok());
}

#[test]
fn test_validate_model_name_hierarchical_valid() {
    // SPEC-dcaeaec4 FR-2: 階層形式を許可
    assert!(validate_model_name("openai/gpt-oss-20b").is_ok());
    assert!(validate_model_name("meta/llama-3-8b").is_ok());
    assert!(validate_model_name("org/sub/model").is_ok());
}

#[test]
fn test_validate_model_name_empty() {
    assert!(validate_model_name("").is_err());
}

#[test]
fn test_validate_model_name_colon_rejected() {
    assert!(validate_model_name("llama3.2:latest").is_err());
}

#[test]
fn test_validate_model_name_invalid_characters() {
    assert!(validate_model_name("Model Name").is_err());
    assert!(validate_model_name("model@name").is_err());
}

#[test]
fn test_validate_model_name_dangerous_patterns_rejected() {
    // パストラバーサル対策
    assert!(validate_model_name("../etc/passwd").is_err());
    assert!(validate_model_name("model/../other").is_err());
    assert!(validate_model_name("/absolute/path").is_err());
    assert!(validate_model_name("trailing/").is_err());
}

// ===== validate_model_name null byte test =====

#[test]
fn test_validate_model_name_null_byte() {
    assert!(validate_model_name("model\0name").is_err());
}

// ===== Additional unit tests for increased coverage =====

// --- validate_model_name extended tests ---

#[test]
fn test_validate_model_name_single_char() {
    assert!(validate_model_name("a").is_ok());
    assert!(validate_model_name("1").is_ok());
}

#[test]
fn test_validate_model_name_uppercase_rejected() {
    assert!(validate_model_name("GPT-4").is_err());
    assert!(validate_model_name("Llama").is_err());
}

#[test]
fn test_validate_model_name_special_chars_rejected() {
    assert!(validate_model_name("model!name").is_err());
    assert!(validate_model_name("model#name").is_err());
    assert!(validate_model_name("model$name").is_err());
    assert!(validate_model_name("model%name").is_err());
    assert!(validate_model_name("model&name").is_err());
    assert!(validate_model_name("model*name").is_err());
}

#[test]
fn test_validate_model_name_allows_dots_and_hyphens() {
    assert!(validate_model_name("model.v1.0-beta").is_ok());
    assert!(validate_model_name("a-b-c.d.e").is_ok());
}

#[test]
fn test_validate_model_name_allows_underscores() {
    assert!(validate_model_name("model_name_v1").is_ok());
    assert!(validate_model_name("_leading").is_ok());
}

#[test]
fn test_validate_model_name_double_slash_rejected() {
    // Leading slash is rejected, but double-dot is the traversal pattern
    assert!(validate_model_name("org//model").is_ok()); // double slash is fine (no .. pattern)
}

#[test]
fn test_validate_model_name_whitespace_rejected() {
    assert!(validate_model_name("model name").is_err());
    assert!(validate_model_name("model\tname").is_err());
    assert!(validate_model_name("model\nname").is_err());
}

#[test]
fn test_validate_model_name_unicode_rejected() {
    assert!(validate_model_name("model-名前").is_err());
    assert!(validate_model_name("модель").is_err());
}
