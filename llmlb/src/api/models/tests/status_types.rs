use super::*;

// ===== SPEC-6cd7f960: 対応モデルリスト型管理 =====

#[test]
fn test_model_status_serialization() {
    // ModelStatusが正しくシリアライズされることを確認
    assert_eq!(
        serde_json::to_string(&ModelStatus::Available).unwrap(),
        "\"available\""
    );
    assert_eq!(
        serde_json::to_string(&ModelStatus::Downloading).unwrap(),
        "\"downloading\""
    );
    assert_eq!(
        serde_json::to_string(&ModelStatus::Downloaded).unwrap(),
        "\"downloaded\""
    );
}

#[test]
fn test_model_with_status_from_registered() {
    let mut model = ModelInfo::new(
        "test-model".to_string(),
        1000,
        "Test Model".to_string(),
        1500,
        vec!["test".to_string()],
    );
    model.repo = Some("test/repo".into());
    model.filename = Some("model.gguf".into());

    let with_status = ModelWithStatus::from_registered(&model);

    assert_eq!(with_status.id, "test-model");
    assert_eq!(with_status.name, "test-model");
    assert_eq!(with_status.description, "Test Model");
    assert_eq!(with_status.status, ModelStatus::Available);
    assert_eq!(
        with_status.lifecycle_status,
        Some(LifecycleStatus::Registered)
    );
    assert!(with_status.download_progress.is_none());
    assert!(with_status.hf_info.is_none());
}

#[test]
fn test_model_with_status_serialization() {
    let mut model = ModelInfo::new(
        "qwen2.5-7b-instruct".to_string(),
        4_920_000_000,
        "Qwen2.5 7B Instruct".to_string(),
        7_380_000_000,
        vec!["chat".to_string()],
    );
    model.repo = Some("bartowski/Qwen2.5-7B-Instruct-GGUF".into());
    model.filename = Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".into());

    let with_status = ModelWithStatus::from_registered(&model);
    let json = serde_json::to_string(&with_status).expect("シリアライズに失敗");

    // JSONに必要なフィールドが含まれることを確認
    assert!(json.contains("\"id\":\"qwen2.5-7b-instruct\""));
    assert!(json.contains("\"status\":\"available\""));
    assert!(json.contains("\"lifecycle_status\":\"registered\""));
    // skip_serializing_if により None フィールドは含まれない
    assert!(!json.contains("\"download_progress\""));
}

#[test]
fn test_hf_info_serialization() {
    let hf_info = HfInfo {
        downloads: Some(125000),
        likes: Some(450),
    };
    let json = serde_json::to_string(&hf_info).expect("シリアライズに失敗");
    assert!(json.contains("\"downloads\":125000"));
    assert!(json.contains("\"likes\":450"));

    // Noneの場合はフィールドが省略される
    let empty_info = HfInfo::default();
    let empty_json = serde_json::to_string(&empty_info).expect("シリアライズに失敗");
    assert_eq!(empty_json, "{}");
}

// ===== LifecycleStatus serialization tests =====

#[test]
fn test_lifecycle_status_serialization() {
    assert_eq!(
        serde_json::to_string(&LifecycleStatus::Pending).unwrap(),
        "\"pending\""
    );
    assert_eq!(
        serde_json::to_string(&LifecycleStatus::Caching).unwrap(),
        "\"caching\""
    );
    assert_eq!(
        serde_json::to_string(&LifecycleStatus::Registered).unwrap(),
        "\"registered\""
    );
    assert_eq!(
        serde_json::to_string(&LifecycleStatus::Error).unwrap(),
        "\"error\""
    );
}

// ===== HfInfo tests =====

#[test]
fn test_hf_info_default_is_empty() {
    let info = HfInfo::default();
    assert!(info.downloads.is_none());
    assert!(info.likes.is_none());
}

#[test]
fn test_hf_info_deserialization() {
    let json = r#"{"downloads": 5000, "likes": 100}"#;
    let info: HfInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.downloads, Some(5000));
    assert_eq!(info.likes, Some(100));
}

#[test]
fn test_hf_info_deserialization_partial() {
    let json = r#"{"downloads": 5000}"#;
    let info: HfInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.downloads, Some(5000));
    assert!(info.likes.is_none());
}

// ===== ModelStatus deserialization tests =====

#[test]
fn test_model_status_deserialization() {
    assert_eq!(
        serde_json::from_str::<ModelStatus>("\"available\"").unwrap(),
        ModelStatus::Available
    );
    assert_eq!(
        serde_json::from_str::<ModelStatus>("\"downloading\"").unwrap(),
        ModelStatus::Downloading
    );
    assert_eq!(
        serde_json::from_str::<ModelStatus>("\"downloaded\"").unwrap(),
        ModelStatus::Downloaded
    );
}

// ===== DownloadProgress serialization =====

#[test]
fn test_download_progress_serialization() {
    let progress = DownloadProgress {
        percent: 0.75,
        bytes_downloaded: Some(750),
        bytes_total: Some(1000),
        error: None,
    };
    let json = serde_json::to_value(&progress).unwrap();
    assert_eq!(json["percent"], 0.75);
    assert_eq!(json["bytes_downloaded"], 750);
    assert_eq!(json["bytes_total"], 1000);
    assert!(json["error"].is_null());
}

#[test]
fn test_download_progress_with_error() {
    let progress = DownloadProgress {
        percent: 0.0,
        bytes_downloaded: None,
        bytes_total: None,
        error: Some("network error".to_string()),
    };
    let json = serde_json::to_value(&progress).unwrap();
    assert_eq!(json["error"], "network error");
}

// --- LifecycleStatus tests ---

#[test]
fn test_lifecycle_status_equality() {
    assert_eq!(LifecycleStatus::Pending, LifecycleStatus::Pending);
    assert_eq!(LifecycleStatus::Caching, LifecycleStatus::Caching);
    assert_eq!(LifecycleStatus::Registered, LifecycleStatus::Registered);
    assert_eq!(LifecycleStatus::Error, LifecycleStatus::Error);
    assert_ne!(LifecycleStatus::Pending, LifecycleStatus::Registered);
}

#[test]
fn test_lifecycle_status_clone() {
    let status = LifecycleStatus::Caching;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_lifecycle_status_debug() {
    let debug = format!("{:?}", LifecycleStatus::Pending);
    assert!(debug.contains("Pending"));
}

// --- ModelStatus tests ---

#[test]
fn test_model_status_equality() {
    assert_eq!(ModelStatus::Available, ModelStatus::Available);
    assert_eq!(ModelStatus::Downloading, ModelStatus::Downloading);
    assert_eq!(ModelStatus::Downloaded, ModelStatus::Downloaded);
    assert_ne!(ModelStatus::Available, ModelStatus::Downloaded);
}

#[test]
fn test_model_status_clone() {
    let status = ModelStatus::Downloading;
    let cloned = status.clone();
    assert_eq!(status, cloned);
}

#[test]
fn test_model_status_roundtrip() {
    for status in [
        ModelStatus::Available,
        ModelStatus::Downloading,
        ModelStatus::Downloaded,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ModelStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_model_status_invalid_deserialization() {
    assert!(serde_json::from_str::<ModelStatus>("\"invalid\"").is_err());
    assert!(serde_json::from_str::<ModelStatus>("\"AVAILABLE\"").is_err());
}

// --- HfInfo extended tests ---

#[test]
fn test_hf_info_clone() {
    let info = HfInfo {
        downloads: Some(100),
        likes: Some(50),
    };
    let cloned = info.clone();
    assert_eq!(cloned.downloads, Some(100));
    assert_eq!(cloned.likes, Some(50));
}

#[test]
fn test_hf_info_debug() {
    let info = HfInfo {
        downloads: Some(100),
        likes: None,
    };
    let debug = format!("{:?}", info);
    assert!(debug.contains("100"));
}

#[test]
fn test_hf_info_roundtrip_with_both_fields() {
    let info = HfInfo {
        downloads: Some(5000),
        likes: Some(200),
    };
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: HfInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.downloads, Some(5000));
    assert_eq!(deserialized.likes, Some(200));
}

#[test]
fn test_hf_info_empty_roundtrip() {
    let info = HfInfo::default();
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: HfInfo = serde_json::from_str(&json).unwrap();
    assert!(deserialized.downloads.is_none());
    assert!(deserialized.likes.is_none());
}

#[test]
fn test_hf_info_deserialization_with_extra_fields() {
    let json = r#"{"downloads": 100, "likes": 50, "extra_field": "ignored"}"#;
    let info: HfInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.downloads, Some(100));
    assert_eq!(info.likes, Some(50));
}

#[test]
fn test_hf_info_deserialization_zero_values() {
    let json = r#"{"downloads": 0, "likes": 0}"#;
    let info: HfInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.downloads, Some(0));
    assert_eq!(info.likes, Some(0));
}

// --- DownloadProgress extended tests ---

#[test]
fn test_download_progress_complete() {
    let progress = DownloadProgress {
        percent: 1.0,
        bytes_downloaded: Some(5000),
        bytes_total: Some(5000),
        error: None,
    };
    let json = serde_json::to_value(&progress).unwrap();
    assert_eq!(json["percent"], 1.0);
    assert_eq!(json["bytes_downloaded"], 5000);
    assert_eq!(json["bytes_total"], 5000);
}

#[test]
fn test_download_progress_zero_percent() {
    let progress = DownloadProgress {
        percent: 0.0,
        bytes_downloaded: Some(0),
        bytes_total: Some(10000),
        error: None,
    };
    let json = serde_json::to_value(&progress).unwrap();
    assert_eq!(json["percent"], 0.0);
}

// --- ModelWithStatus extended tests ---

#[test]
fn test_model_with_status_default_fields() {
    let model = ModelInfo::new("test".to_string(), 0, "desc".to_string(), 0, vec![]);
    let with_status = ModelWithStatus::from_registered(&model);
    assert_eq!(with_status.repo, "");
    assert_eq!(with_status.recommended_filename, "");
    assert_eq!(with_status.size_bytes, 0);
    assert_eq!(with_status.required_memory_bytes, 0);
    assert!(with_status.quantization.is_none());
    assert!(with_status.parameter_count.is_none());
}

// --- ArtifactFormat tests ---

#[test]
fn test_artifact_format_equality() {
    assert_eq!(ArtifactFormat::Gguf, ArtifactFormat::Gguf);
    assert_eq!(ArtifactFormat::Safetensors, ArtifactFormat::Safetensors);
    assert_ne!(ArtifactFormat::Gguf, ArtifactFormat::Safetensors);
}

#[test]
fn test_artifact_format_copy() {
    let fmt = ArtifactFormat::Gguf;
    let copied = fmt;
    assert_eq!(fmt, copied);
}
