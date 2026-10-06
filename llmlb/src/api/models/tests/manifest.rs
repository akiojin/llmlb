use super::*;

// ===== manifest_format_label tests =====

#[test]
fn test_manifest_format_label_gguf() {
    assert_eq!(manifest_format_label(ArtifactFormat::Gguf), "gguf");
}

#[test]
fn test_manifest_format_label_safetensors() {
    assert_eq!(
        manifest_format_label(ArtifactFormat::Safetensors),
        "safetensors"
    );
}

// ===== manifest_file_priority tests =====

#[test]
fn test_manifest_file_priority_config() {
    assert_eq!(manifest_file_priority("config.json"), Some(10));
}

#[test]
fn test_manifest_file_priority_tokenizer() {
    assert_eq!(manifest_file_priority("tokenizer.json"), Some(10));
}

#[test]
fn test_manifest_file_priority_safetensors_index() {
    assert_eq!(
        manifest_file_priority("model.safetensors.index.json"),
        Some(5)
    );
}

#[test]
fn test_manifest_file_priority_metal() {
    assert_eq!(manifest_file_priority("model.metal.bin"), Some(5));
}

#[test]
fn test_manifest_file_priority_regular_file() {
    assert_eq!(manifest_file_priority("model.gguf"), None);
    assert_eq!(manifest_file_priority("model.safetensors"), None);
}

// ===== is_quantization_token tests =====

#[test]
fn test_is_quantization_token_q_variants() {
    assert!(is_quantization_token("Q4_K_M"));
    assert!(is_quantization_token("Q5_K_S"));
    assert!(is_quantization_token("Q8_0"));
    assert!(is_quantization_token("Q2_K"));
}

#[test]
fn test_is_quantization_token_iq_variants() {
    assert!(is_quantization_token("IQ4_XS"));
    assert!(is_quantization_token("IQ2_S"));
}

#[test]
fn test_is_quantization_token_fp_bf_variants() {
    assert!(is_quantization_token("FP16"));
    assert!(is_quantization_token("BF16"));
    assert!(is_quantization_token("F32"));
    assert!(is_quantization_token("F16"));
}

#[test]
fn test_is_quantization_token_mx_variants() {
    assert!(is_quantization_token("MXFP4"));
    assert!(is_quantization_token("MX8"));
}

#[test]
fn test_is_quantization_token_non_quant() {
    assert!(!is_quantization_token("instruct"));
    assert!(!is_quantization_token("llama"));
    assert!(!is_quantization_token(""));
    assert!(!is_quantization_token("Q"));
    assert!(!is_quantization_token("F"));
}

// ===== infer_quantization_from_filename tests =====

#[test]
fn test_infer_quantization_from_filename_standard() {
    assert_eq!(
        infer_quantization_from_filename("Llama-2-7B-Q4_K_M.gguf"),
        Some("Q4_K_M".to_string())
    );
}

#[test]
fn test_infer_quantization_from_filename_with_dots() {
    assert_eq!(
        infer_quantization_from_filename("model.Q5_K_S.gguf"),
        Some("Q5_K_S".to_string())
    );
}

#[test]
fn test_infer_quantization_from_filename_fp16() {
    assert_eq!(
        infer_quantization_from_filename("model-FP16.gguf"),
        Some("FP16".to_string())
    );
}

#[test]
fn test_infer_quantization_from_filename_no_quant() {
    // Model without a quantization token
    assert_eq!(infer_quantization_from_filename("llama-model.gguf"), None);
}

#[test]
fn test_infer_quantization_from_filename_non_gguf() {
    assert_eq!(infer_quantization_from_filename("model.safetensors"), None);
}

#[test]
fn test_infer_quantization_from_filename_with_path() {
    assert_eq!(
        infer_quantization_from_filename("subdir/model-Q4_K_M.gguf"),
        Some("Q4_K_M".to_string())
    );
}

// ===== extract_runtime_from_config tests =====

#[test]
fn test_extract_runtime_gptoss_architecture() {
    let config = serde_json::json!({
        "architectures": ["GptOssForCausalLM"]
    });
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("gptoss_cpp".to_string())
    );
}

#[test]
fn test_extract_runtime_gptoss_uppercase() {
    let config = serde_json::json!({
        "architectures": ["GPTOSSModel"]
    });
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("gptoss_cpp".to_string())
    );
}

#[test]
fn test_extract_runtime_nemotron_architecture() {
    let config = serde_json::json!({
        "architectures": ["NemotronForCausalLM"]
    });
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("nemotron_cpp".to_string())
    );
}

#[test]
fn test_extract_runtime_from_model_type_gptoss() {
    let config = serde_json::json!({
        "model_type": "gpt_oss"
    });
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("gptoss_cpp".to_string())
    );
}

#[test]
fn test_extract_runtime_from_model_type_nemotron() {
    let config = serde_json::json!({
        "model_type": "nemotron"
    });
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("nemotron_cpp".to_string())
    );
}

#[test]
fn test_extract_runtime_unknown_architecture() {
    let config = serde_json::json!({
        "architectures": ["LlamaForCausalLM"]
    });
    assert_eq!(extract_runtime_from_config(&config), None);
}

#[test]
fn test_extract_runtime_empty_config() {
    let config = serde_json::json!({});
    assert_eq!(extract_runtime_from_config(&config), None);
}

// --- manifest_file_priority edge cases ---

#[test]
fn test_manifest_file_priority_empty_string() {
    assert_eq!(manifest_file_priority(""), None);
}

#[test]
fn test_manifest_file_priority_case_sensitive() {
    // config.json / tokenizer.json checks are exact match
    assert_eq!(manifest_file_priority("Config.json"), None);
    assert_eq!(manifest_file_priority("TOKENIZER.JSON"), None);
}

// --- is_quantization_token extended tests ---

#[test]
fn test_is_quantization_token_with_special_chars() {
    assert!(!is_quantization_token("Q4-K-M")); // hyphens not allowed
    assert!(!is_quantization_token("Q4.K.M")); // dots not allowed
}

#[test]
fn test_is_quantization_token_single_letter_no_digit() {
    assert!(!is_quantization_token("Q"));
    assert!(!is_quantization_token("F"));
    assert!(!is_quantization_token("BF"));
    assert!(!is_quantization_token("FP"));
    assert!(!is_quantization_token("IQ"));
    assert!(!is_quantization_token("MX"));
}

#[test]
fn test_is_quantization_token_mx_needs_digit() {
    assert!(is_quantization_token("MX4")); // has digit
    assert!(!is_quantization_token("MXAB")); // no digit
}

// --- infer_quantization_from_filename edge cases ---

#[test]
fn test_infer_quantization_from_filename_empty() {
    assert_eq!(infer_quantization_from_filename(""), None);
}

#[test]
fn test_infer_quantization_from_filename_just_gguf() {
    assert_eq!(infer_quantization_from_filename(".gguf"), None);
}

#[test]
fn test_infer_quantization_from_filename_iq_variant() {
    assert_eq!(
        infer_quantization_from_filename("model-IQ4_XS.gguf"),
        Some("IQ4_XS".to_string())
    );
}

#[test]
fn test_infer_quantization_from_filename_bf16() {
    assert_eq!(
        infer_quantization_from_filename("model-BF16.gguf"),
        Some("BF16".to_string())
    );
}

// --- extract_runtime_from_config edge cases ---

#[test]
fn test_extract_runtime_architectures_empty_array() {
    let config = serde_json::json!({
        "architectures": []
    });
    assert_eq!(extract_runtime_from_config(&config), None);
}

#[test]
fn test_extract_runtime_architectures_non_string_elements() {
    let config = serde_json::json!({
        "architectures": [42, null, true]
    });
    assert_eq!(extract_runtime_from_config(&config), None);
}

#[test]
fn test_extract_runtime_model_type_unknown() {
    let config = serde_json::json!({
        "model_type": "llama"
    });
    assert_eq!(extract_runtime_from_config(&config), None);
}

#[test]
fn test_extract_runtime_architectures_take_priority_over_model_type() {
    let config = serde_json::json!({
        "architectures": ["GptOssForCausalLM"],
        "model_type": "nemotron"
    });
    // architectures should be checked first, so gptoss_cpp should be returned
    assert_eq!(
        extract_runtime_from_config(&config),
        Some("gptoss_cpp".to_string())
    );
}

// --- manifest_format_label tests ---

#[test]
fn test_manifest_format_label_returns_static_str() {
    let gguf_label = manifest_format_label(ArtifactFormat::Gguf);
    let st_label = manifest_format_label(ArtifactFormat::Safetensors);
    assert!(!gguf_label.is_empty());
    assert!(!st_label.is_empty());
    assert_ne!(gguf_label, st_label);
}

// --- ManifestFile serialization ---

#[test]
fn test_manifest_file_serialization_minimal() {
    let f = ManifestFile {
        name: "model.gguf".to_string(),
        priority: None,
        runtimes: None,
        url: None,
        optional: None,
    };
    let json = serde_json::to_value(&f).unwrap();
    assert_eq!(json["name"], "model.gguf");
    // skip_serializing_if fields should not be present
    assert!(json.get("priority").is_none());
    assert!(json.get("runtimes").is_none());
    assert!(json.get("url").is_none());
    assert!(json.get("optional").is_none());
}

#[test]
fn test_manifest_file_serialization_full() {
    let f = ManifestFile {
        name: "config.json".to_string(),
        priority: Some(10),
        runtimes: Some(vec!["llama_cpp".to_string()]),
        url: Some("https://example.com/config.json".to_string()),
        optional: Some(true),
    };
    let json = serde_json::to_value(&f).unwrap();
    assert_eq!(json["name"], "config.json");
    assert_eq!(json["priority"], 10);
    assert_eq!(json["runtimes"][0], "llama_cpp");
    assert_eq!(json["url"], "https://example.com/config.json");
    assert_eq!(json["optional"], true);
}

// --- Manifest serialization ---

#[test]
fn test_manifest_serialization() {
    let manifest = Manifest {
        format: "gguf".to_string(),
        files: vec![ManifestFile {
            name: "model.gguf".to_string(),
            priority: None,
            runtimes: Some(vec!["llama_cpp".to_string()]),
            url: Some("https://example.com/model.gguf".to_string()),
            optional: None,
        }],
        quantization: Some("Q4_K_M".to_string()),
    };
    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(json["format"], "gguf");
    assert_eq!(json["files"].as_array().unwrap().len(), 1);
    assert_eq!(json["quantization"], "Q4_K_M");
}

#[test]
fn test_manifest_serialization_no_quantization() {
    let manifest = Manifest {
        format: "safetensors".to_string(),
        files: vec![],
        quantization: None,
    };
    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(json["format"], "safetensors");
    assert!(json.get("quantization").is_none());
}
