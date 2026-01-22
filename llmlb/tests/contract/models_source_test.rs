//! Contract Test: ModelSource enum variants
//!
//! SPEC-1970e39f: 構造化ロギング強化
//! models.jsonの`hf_onnx`バリアント対応

use llmlb::registry::models::ModelSource;

/// T005: ModelSourceがhf_onnxをデシリアライズできることを検証
#[test]
fn test_model_source_deserializes_hf_onnx() {
    let json = r#""hf_onnx""#;
    let result: Result<ModelSource, _> = serde_json::from_str(json);

    assert!(
        result.is_ok(),
        "ModelSource should deserialize 'hf_onnx': {:?}",
        result.err()
    );

    let source = result.unwrap();
    assert!(
        matches!(source, ModelSource::HfOnnx),
        "Deserialized value should be HfOnnx variant"
    );
}

/// ModelSourceの全バリアントがシリアライズ/デシリアライズできることを検証
#[test]
fn test_model_source_all_variants_roundtrip() {
    let variants = vec![
        (ModelSource::Predefined, "predefined"),
        (ModelSource::HfGguf, "hf_gguf"),
        (ModelSource::HfSafetensors, "hf_safetensors"),
        (ModelSource::HfOnnx, "hf_onnx"),
    ];

    for (variant, expected_str) in variants {
        // シリアライズ
        let serialized = serde_json::to_string(&variant).expect("serialize should succeed");
        assert_eq!(
            serialized,
            format!("\"{}\"", expected_str),
            "Serialized form should match"
        );

        // デシリアライズ
        let deserialized: ModelSource =
            serde_json::from_str(&serialized).expect("deserialize should succeed");
        assert_eq!(
            deserialized, variant,
            "Roundtrip should preserve the variant"
        );
    }
}

/// ModelInfoにhf_onnxソースを持つモデルがデシリアライズできることを検証
#[test]
fn test_model_info_with_hf_onnx_source() {
    let json = r#"{
        "name": "whisper-large-v3",
        "size": 1500000000,
        "description": "Whisper Large V3 ONNX model",
        "required_memory": 2000000000,
        "tags": ["audio", "transcription"],
        "source": "hf_onnx"
    }"#;

    let result: Result<llmlb::registry::models::ModelInfo, _> = serde_json::from_str(json);

    assert!(
        result.is_ok(),
        "ModelInfo with hf_onnx source should deserialize: {:?}",
        result.err()
    );

    let model_info = result.unwrap();
    assert_eq!(model_info.name, "whisper-large-v3");
    assert!(
        matches!(model_info.source, ModelSource::HfOnnx),
        "Source should be HfOnnx"
    );
}
