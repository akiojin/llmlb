//! SPEC-e8e9326e: エンドポイントタイプ判別ロジックのUnit Test
//!
//! T138: タイプ判別ロジックのunit test

use llmlb::types::endpoint::EndpointType;

/// xLLMエンドポイントはモデルダウンロードをサポート
#[test]
fn test_xllm_supports_model_download() {
    assert!(EndpointType::Xllm.supports_model_download());
}

/// ダウンロード対応: xLLM, Ollama, LM Studio
#[test]
fn test_download_supported_endpoints() {
    assert!(EndpointType::Ollama.supports_model_download());
    assert!(EndpointType::LmStudio.supports_model_download());
}

/// ダウンロード非対応: vLLM, llama.cpp, OpenAI互換
#[test]
fn test_download_unsupported_endpoints() {
    assert!(!EndpointType::Vllm.supports_model_download());
    assert!(!EndpointType::Llamacpp.supports_model_download());
    assert!(!EndpointType::OpenaiCompatible.supports_model_download());
}

/// 削除対応: Ollama のみ
#[test]
fn test_delete_supported_endpoints() {
    assert!(!EndpointType::Xllm.supports_model_delete());
    assert!(EndpointType::Ollama.supports_model_delete());
    assert!(!EndpointType::LmStudio.supports_model_delete());
    assert!(!EndpointType::Vllm.supports_model_delete());
    assert!(!EndpointType::Llamacpp.supports_model_delete());
    assert!(!EndpointType::OpenaiCompatible.supports_model_delete());
}

/// xLLM、Ollama、LmStudioはモデルメタデータ取得をサポート
#[test]
fn test_metadata_support() {
    assert!(EndpointType::Xllm.supports_model_metadata());
    assert!(EndpointType::Ollama.supports_model_metadata());
    assert!(EndpointType::LmStudio.supports_model_metadata());
    assert!(!EndpointType::Vllm.supports_model_metadata());
    assert!(!EndpointType::Llamacpp.supports_model_metadata());
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
        EndpointType::Llamacpp,
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
