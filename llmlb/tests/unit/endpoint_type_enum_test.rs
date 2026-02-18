//! SPEC-e8e9326e: EndpointType列挙型のシリアライズ/デシリアライズテスト
//!
//! T139: EndpointType列挙型のunit test

use llmlb::types::endpoint::EndpointType;

/// JSON シリアライズ: snake_case形式
#[test]
fn test_json_serialization() {
    assert_eq!(
        serde_json::to_string(&EndpointType::Xllm).unwrap(),
        "\"xllm\""
    );
    assert_eq!(
        serde_json::to_string(&EndpointType::Ollama).unwrap(),
        "\"ollama\""
    );
    assert_eq!(
        serde_json::to_string(&EndpointType::Vllm).unwrap(),
        "\"vllm\""
    );
    assert_eq!(
        serde_json::to_string(&EndpointType::LmStudio).unwrap(),
        "\"lm_studio\""
    );
    assert_eq!(
        serde_json::to_string(&EndpointType::OpenaiCompatible).unwrap(),
        "\"openai_compatible\""
    );
}

/// JSON デシリアライズ: snake_case形式
#[test]
fn test_json_deserialization() {
    assert_eq!(
        serde_json::from_str::<EndpointType>("\"xllm\"").unwrap(),
        EndpointType::Xllm
    );
    assert_eq!(
        serde_json::from_str::<EndpointType>("\"ollama\"").unwrap(),
        EndpointType::Ollama
    );
    assert_eq!(
        serde_json::from_str::<EndpointType>("\"vllm\"").unwrap(),
        EndpointType::Vllm
    );
    assert_eq!(
        serde_json::from_str::<EndpointType>("\"lm_studio\"").unwrap(),
        EndpointType::LmStudio
    );
    assert_eq!(
        serde_json::from_str::<EndpointType>("\"openai_compatible\"").unwrap(),
        EndpointType::OpenaiCompatible
    );
}

/// JSON ラウンドトリップ
#[test]
fn test_json_roundtrip() {
    let types = [
        EndpointType::Xllm,
        EndpointType::Ollama,
        EndpointType::Vllm,
        EndpointType::LmStudio,
        EndpointType::OpenaiCompatible,
    ];

    for original in types {
        let json = serde_json::to_string(&original).unwrap();
        let parsed: EndpointType = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}

/// FromStr: 正常系
#[test]
fn test_from_str_valid() {
    assert_eq!("xllm".parse::<EndpointType>().unwrap(), EndpointType::Xllm);
    assert_eq!(
        "ollama".parse::<EndpointType>().unwrap(),
        EndpointType::Ollama
    );
    assert_eq!("vllm".parse::<EndpointType>().unwrap(), EndpointType::Vllm);
    assert_eq!(
        "lm_studio".parse::<EndpointType>().unwrap(),
        EndpointType::LmStudio
    );
    assert_eq!(
        "openai_compatible".parse::<EndpointType>().unwrap(),
        EndpointType::OpenaiCompatible
    );
}

/// FromStr: 不正値はエラーを返す
#[test]
fn test_from_str_invalid_returns_error() {
    assert!("invalid_type".parse::<EndpointType>().is_err());
    assert!("".parse::<EndpointType>().is_err());
    assert!("XLLM".parse::<EndpointType>().is_err());
    assert!("Ollama".parse::<EndpointType>().is_err());
    assert!("unknown".parse::<EndpointType>().is_err());
}

/// Display: as_str()と一致
#[test]
fn test_display() {
    let types = [
        EndpointType::Xllm,
        EndpointType::Ollama,
        EndpointType::Vllm,
        EndpointType::LmStudio,
        EndpointType::OpenaiCompatible,
    ];

    for t in types {
        assert_eq!(format!("{}", t), t.as_str());
    }
}

/// as_str: 各バリアントの文字列表現
#[test]
fn test_as_str() {
    assert_eq!(EndpointType::Xllm.as_str(), "xllm");
    assert_eq!(EndpointType::Ollama.as_str(), "ollama");
    assert_eq!(EndpointType::Vllm.as_str(), "vllm");
    assert_eq!(EndpointType::LmStudio.as_str(), "lm_studio");
    assert_eq!(EndpointType::OpenaiCompatible.as_str(), "openai_compatible");
}

/// Clone と Copy
#[test]
fn test_clone_copy() {
    let t1 = EndpointType::Xllm;
    let t2 = t1; // Copy
    let t3 = t1; // Copy

    assert_eq!(t1, t2);
    assert_eq!(t1, t3);
}

/// PartialEq
#[test]
fn test_partial_eq() {
    assert_eq!(EndpointType::Xllm, EndpointType::Xllm);
    assert_ne!(EndpointType::Xllm, EndpointType::Ollama);
}

/// Endpointに含まれるEndpointTypeのシリアライズ
#[test]
fn test_endpoint_type_in_endpoint_json() {
    use llmlb::types::endpoint::Endpoint;

    let endpoint = Endpoint::new(
        "Test".to_string(),
        "http://localhost:8080".to_string(),
        EndpointType::Xllm,
    );

    let json = serde_json::to_string(&endpoint).unwrap();
    assert!(json.contains("\"endpoint_type\":\"xllm\""));
}
