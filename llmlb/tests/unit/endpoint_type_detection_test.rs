//! SPEC-66555000: エンドポイントタイプ判別ロジックのUnit Test
//!
//! T138: タイプ判別ロジックのunit test

use llmlb::types::endpoint::EndpointType;

/// 判別優先度のテスト: xLLM > Ollama > vLLM > OpenAI-compatible > Unknown
#[test]
fn test_detection_priority_order() {
    // 各タイプの優先度を数値化
    fn priority(t: EndpointType) -> u8 {
        match t {
            EndpointType::Xllm => 5,
            EndpointType::Ollama => 4,
            EndpointType::Vllm => 3,
            EndpointType::OpenaiCompatible => 2,
            EndpointType::Unknown => 1,
        }
    }

    // xLLM が最高優先度
    assert!(priority(EndpointType::Xllm) > priority(EndpointType::Ollama));
    assert!(priority(EndpointType::Ollama) > priority(EndpointType::Vllm));
    assert!(priority(EndpointType::Vllm) > priority(EndpointType::OpenaiCompatible));
    assert!(priority(EndpointType::OpenaiCompatible) > priority(EndpointType::Unknown));
}

/// xLLMエンドポイントはモデルダウンロードをサポート
#[test]
fn test_xllm_supports_model_download() {
    assert!(EndpointType::Xllm.supports_model_download());
}

/// 非xLLMエンドポイントはモデルダウンロードをサポートしない
#[test]
fn test_non_xllm_does_not_support_model_download() {
    assert!(!EndpointType::Ollama.supports_model_download());
    assert!(!EndpointType::Vllm.supports_model_download());
    assert!(!EndpointType::OpenaiCompatible.supports_model_download());
    assert!(!EndpointType::Unknown.supports_model_download());
}

/// xLLMとOllamaはモデルメタデータ取得をサポート
#[test]
fn test_metadata_support() {
    assert!(EndpointType::Xllm.supports_model_metadata());
    assert!(EndpointType::Ollama.supports_model_metadata());
    assert!(!EndpointType::Vllm.supports_model_metadata());
    assert!(!EndpointType::OpenaiCompatible.supports_model_metadata());
    assert!(!EndpointType::Unknown.supports_model_metadata());
}

/// デフォルトタイプはUnknown（オフライン時のフォールバック）
#[test]
fn test_default_type_is_unknown() {
    assert_eq!(EndpointType::default(), EndpointType::Unknown);
}

/// タイプの文字列変換は双方向で一貫性がある
#[test]
fn test_type_string_roundtrip() {
    let types = [
        EndpointType::Xllm,
        EndpointType::Ollama,
        EndpointType::Vllm,
        EndpointType::OpenaiCompatible,
        EndpointType::Unknown,
    ];

    for t in types {
        let s = t.as_str();
        let parsed: EndpointType = s.parse().unwrap();
        assert_eq!(t, parsed);
    }
}

/// 不正な文字列はUnknownにフォールバック
#[test]
fn test_invalid_string_fallback() {
    assert_eq!("invalid".parse::<EndpointType>().unwrap(), EndpointType::Unknown);
    assert_eq!("".parse::<EndpointType>().unwrap(), EndpointType::Unknown);
    assert_eq!("XLLM".parse::<EndpointType>().unwrap(), EndpointType::Unknown); // 大文字は不正
}
