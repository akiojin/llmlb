//! SPEC-e8e9326e: エンドポイントタイプ判別ロジックのUnit Test
//!
//! T138: タイプ判別ロジックのunit test

use llmlb::types::endpoint::EndpointType;

/// 判別優先度のテスト: xLLM > Ollama > vLLM > OpenAI-compatible
#[test]
fn test_detection_priority_order() {
    // 各タイプの優先度を数値化
    fn priority(t: EndpointType) -> u8 {
        match t {
            EndpointType::Xllm => 5,
            EndpointType::Ollama => 4,
            EndpointType::LmStudio => 3,
            EndpointType::Vllm => 2,
            EndpointType::OpenaiCompatible => 1,
        }
    }

    // xLLM が最高優先度
    assert!(priority(EndpointType::Xllm) > priority(EndpointType::Ollama));
    assert!(priority(EndpointType::Ollama) > priority(EndpointType::LmStudio));
    assert!(priority(EndpointType::LmStudio) > priority(EndpointType::Vllm));
    assert!(priority(EndpointType::Vllm) > priority(EndpointType::OpenaiCompatible));
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
    assert!(!EndpointType::LmStudio.supports_model_download());
    assert!(!EndpointType::OpenaiCompatible.supports_model_download());
}

/// xLLM、Ollama、LmStudioはモデルメタデータ取得をサポート
#[test]
fn test_metadata_support() {
    assert!(EndpointType::Xllm.supports_model_metadata());
    assert!(EndpointType::Ollama.supports_model_metadata());
    assert!(EndpointType::LmStudio.supports_model_metadata());
    assert!(!EndpointType::Vllm.supports_model_metadata());
    assert!(!EndpointType::OpenaiCompatible.supports_model_metadata());
}

/// タイプの文字列変換は双方向で一貫性がある
#[test]
fn test_type_string_roundtrip() {
    let types = [
        EndpointType::Xllm,
        EndpointType::Ollama,
        EndpointType::Vllm,
        EndpointType::LmStudio,
        EndpointType::OpenaiCompatible,
    ];

    for t in types {
        let s = t.as_str();
        let parsed: EndpointType = s.parse().unwrap();
        assert_eq!(t, parsed);
    }
}

/// 不正な文字列はエラーを返す
#[test]
fn test_invalid_string_returns_error() {
    assert!("invalid".parse::<EndpointType>().is_err());
    assert!("".parse::<EndpointType>().is_err());
    assert!("XLLM".parse::<EndpointType>().is_err()); // 大文字は不正
    assert!("unknown".parse::<EndpointType>().is_err());
}
