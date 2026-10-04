use super::*;

// ===== is_gguf_filename tests =====

#[test]
fn test_is_gguf_filename_valid() {
    assert!(is_gguf_filename("model.gguf"));
    assert!(is_gguf_filename("Llama-2-7B-Q4_K_M.gguf"));
    assert!(is_gguf_filename("MODEL.GGUF"));
    assert!(is_gguf_filename("path/to/model.gguf"));
}

#[test]
fn test_is_gguf_filename_invalid() {
    assert!(!is_gguf_filename("model.safetensors"));
    assert!(!is_gguf_filename("model.bin"));
    assert!(!is_gguf_filename("model"));
    assert!(!is_gguf_filename(""));
    assert!(!is_gguf_filename("gguf"));
}

// ===== is_safetensors_filename tests =====

#[test]
fn test_is_safetensors_filename_valid() {
    assert!(is_safetensors_filename("model.safetensors"));
    assert!(is_safetensors_filename("model.safetensors.index.json"));
    assert!(is_safetensors_filename("MODEL.SAFETENSORS"));
    assert!(is_safetensors_filename("model.SAFETENSORS.INDEX.JSON"));
}

#[test]
fn test_is_safetensors_filename_invalid() {
    assert!(!is_safetensors_filename("model.gguf"));
    assert!(!is_safetensors_filename("model.bin"));
    assert!(!is_safetensors_filename("safetensors"));
    assert!(!is_safetensors_filename(""));
}

// ===== is_safetensors_index_filename tests =====

#[test]
fn test_is_safetensors_index_filename_valid() {
    assert!(is_safetensors_index_filename(
        "model.safetensors.index.json"
    ));
    assert!(is_safetensors_index_filename(
        "MODEL.SAFETENSORS.INDEX.JSON"
    ));
}

#[test]
fn test_is_safetensors_index_filename_invalid() {
    assert!(!is_safetensors_index_filename("model.safetensors"));
    assert!(!is_safetensors_index_filename("model.gguf"));
    assert!(!is_safetensors_index_filename("index.json"));
}

// ===== infer_safetensors_index_from_shard tests =====

#[test]
fn test_infer_index_from_shard_standard() {
    let result = infer_safetensors_index_from_shard("model-00001-of-00003.safetensors");
    assert_eq!(result, Some("model.safetensors.index.json".to_string()));
}

#[test]
fn test_infer_index_from_shard_with_directory() {
    let result = infer_safetensors_index_from_shard("subdir/model-00001-of-00003.safetensors");
    assert_eq!(
        result,
        Some("subdir/model.safetensors.index.json".to_string())
    );
}

#[test]
fn test_infer_index_from_shard_not_a_shard() {
    assert!(infer_safetensors_index_from_shard("model.safetensors").is_none());
}

#[test]
fn test_infer_index_from_shard_index_file_returns_none() {
    assert!(infer_safetensors_index_from_shard("model.safetensors.index.json").is_none());
}

#[test]
fn test_infer_index_from_shard_non_safetensors() {
    assert!(infer_safetensors_index_from_shard("model.gguf").is_none());
}

// ===== validate_artifact_path tests =====

#[test]
fn test_validate_artifact_path_valid() {
    assert!(validate_artifact_path("model.gguf").is_ok());
    assert!(validate_artifact_path("subdir/model.safetensors").is_ok());
}

#[test]
fn test_validate_artifact_path_empty() {
    assert!(validate_artifact_path("").is_err());
}

#[test]
fn test_validate_artifact_path_traversal() {
    assert!(validate_artifact_path("../etc/passwd").is_err());
    assert!(validate_artifact_path("model/../other.gguf").is_err());
}

#[test]
fn test_validate_artifact_path_null_byte() {
    assert!(validate_artifact_path("model\0.gguf").is_err());
}

#[test]
fn test_validate_artifact_path_absolute() {
    assert!(validate_artifact_path("/absolute/path.gguf").is_err());
    assert!(validate_artifact_path("\\windows\\path.gguf").is_err());
}

// ===== sibling_size_bytes tests =====

#[test]
fn test_sibling_size_bytes_direct_size() {
    let s = HfSibling {
        rfilename: "model.gguf".to_string(),
        size: Some(1000),
        lfs: None,
    };
    assert_eq!(sibling_size_bytes(&s), 1000);
}

#[test]
fn test_sibling_size_bytes_lfs_size() {
    let s = HfSibling {
        rfilename: "model.gguf".to_string(),
        size: None,
        lfs: Some(HfLfs { size: Some(2000) }),
    };
    assert_eq!(sibling_size_bytes(&s), 2000);
}

#[test]
fn test_sibling_size_bytes_prefers_direct() {
    let s = HfSibling {
        rfilename: "model.gguf".to_string(),
        size: Some(500),
        lfs: Some(HfLfs { size: Some(1000) }),
    };
    assert_eq!(sibling_size_bytes(&s), 500);
}

#[test]
fn test_sibling_size_bytes_no_size() {
    let s = HfSibling {
        rfilename: "model.gguf".to_string(),
        size: None,
        lfs: None,
    };
    assert_eq!(sibling_size_bytes(&s), 0);
}

// ===== has_sibling tests =====

#[test]
fn test_has_sibling_found() {
    let siblings = vec![
        HfSibling {
            rfilename: "config.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model.gguf".to_string(),
            size: None,
            lfs: None,
        },
    ];
    assert!(has_sibling(&siblings, "model.gguf"));
    assert!(has_sibling(&siblings, "config.json"));
}

#[test]
fn test_has_sibling_not_found() {
    let siblings = vec![HfSibling {
        rfilename: "config.json".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(!has_sibling(&siblings, "model.gguf"));
}

#[test]
fn test_has_sibling_empty() {
    let siblings: Vec<HfSibling> = vec![];
    assert!(!has_sibling(&siblings, "anything"));
}

// ===== find_metal_artifact tests =====

#[test]
fn test_find_metal_artifact_standard() {
    let siblings = vec![HfSibling {
        rfilename: "model.metal.bin".to_string(),
        size: None,
        lfs: None,
    }];
    assert_eq!(
        find_metal_artifact(&siblings),
        Some("model.metal.bin".to_string())
    );
}

#[test]
fn test_find_metal_artifact_subdirectory() {
    let siblings = vec![HfSibling {
        rfilename: "metal/model.bin".to_string(),
        size: None,
        lfs: None,
    }];
    assert_eq!(
        find_metal_artifact(&siblings),
        Some("metal/model.bin".to_string())
    );
}

#[test]
fn test_find_metal_artifact_none() {
    let siblings = vec![HfSibling {
        rfilename: "model.gguf".to_string(),
        size: None,
        lfs: None,
    }];
    assert_eq!(find_metal_artifact(&siblings), None);
}

// ===== require_safetensors_metadata_files tests =====

#[test]
fn test_require_safetensors_metadata_both_present() {
    let siblings = vec![
        HfSibling {
            rfilename: "config.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "tokenizer.json".to_string(),
            size: None,
            lfs: None,
        },
    ];
    assert!(require_safetensors_metadata_files(&siblings).is_ok());
}

#[test]
fn test_require_safetensors_metadata_missing_config() {
    let siblings = vec![HfSibling {
        rfilename: "tokenizer.json".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(require_safetensors_metadata_files(&siblings).is_err());
}

#[test]
fn test_require_safetensors_metadata_missing_tokenizer() {
    let siblings = vec![HfSibling {
        rfilename: "config.json".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(require_safetensors_metadata_files(&siblings).is_err());
}

#[test]
fn test_require_safetensors_metadata_both_missing() {
    let siblings: Vec<HfSibling> = vec![];
    assert!(require_safetensors_metadata_files(&siblings).is_err());
}

// ===== resolve_primary_artifact tests =====

#[test]
fn test_resolve_primary_artifact_single_gguf() {
    let siblings = vec![HfSibling {
        rfilename: "model.gguf".to_string(),
        size: Some(1000),
        lfs: None,
    }];
    let sel = resolve_primary_artifact(&siblings, None).unwrap();
    assert_eq!(sel.format, ArtifactFormat::Gguf);
    assert_eq!(sel.filename, "model.gguf");
}

#[test]
fn test_resolve_primary_artifact_multiple_gguf_no_hint_error() {
    let siblings = vec![
        HfSibling {
            rfilename: "model-Q4.gguf".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model-Q8.gguf".to_string(),
            size: None,
            lfs: None,
        },
    ];
    assert!(resolve_primary_artifact(&siblings, None).is_err());
}

#[test]
fn test_resolve_primary_artifact_with_gguf_hint() {
    let siblings = vec![
        HfSibling {
            rfilename: "model-Q4.gguf".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model-Q8.gguf".to_string(),
            size: None,
            lfs: None,
        },
    ];
    let sel = resolve_primary_artifact(&siblings, Some("model-Q4.gguf".to_string())).unwrap();
    assert_eq!(sel.format, ArtifactFormat::Gguf);
    assert_eq!(sel.filename, "model-Q4.gguf");
}

#[test]
fn test_resolve_primary_artifact_hint_not_found() {
    let siblings = vec![HfSibling {
        rfilename: "model.gguf".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(resolve_primary_artifact(&siblings, Some("nonexistent.gguf".to_string()),).is_err());
}

#[test]
fn test_resolve_primary_artifact_no_artifacts() {
    let siblings = vec![HfSibling {
        rfilename: "README.md".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(resolve_primary_artifact(&siblings, None).is_err());
}

#[test]
fn test_resolve_primary_artifact_mixed_formats_no_hint_error() {
    let siblings = vec![
        HfSibling {
            rfilename: "model.gguf".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model.safetensors".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "config.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "tokenizer.json".to_string(),
            size: None,
            lfs: None,
        },
    ];
    assert!(resolve_primary_artifact(&siblings, None).is_err());
}

// ===== resolve_primary_artifact with safetensors hint =====

#[test]
fn test_resolve_primary_artifact_safetensors_single() {
    let siblings = vec![
        HfSibling {
            rfilename: "config.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "tokenizer.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model.safetensors".to_string(),
            size: Some(4000),
            lfs: None,
        },
    ];
    let sel = resolve_primary_artifact(&siblings, None).unwrap();
    assert_eq!(sel.format, ArtifactFormat::Safetensors);
    assert_eq!(sel.filename, "model.safetensors");
}

#[test]
fn test_resolve_primary_artifact_unsupported_hint_extension() {
    let siblings = vec![HfSibling {
        rfilename: "model.bin".to_string(),
        size: None,
        lfs: None,
    }];
    assert!(resolve_primary_artifact(&siblings, Some("model.bin".to_string()),).is_err());
}

// --- is_gguf_filename edge cases ---

#[test]
fn test_is_gguf_filename_mixed_case() {
    assert!(is_gguf_filename("model.GgUf"));
    assert!(is_gguf_filename("model.Gguf"));
}

#[test]
fn test_is_gguf_filename_dot_gguf_only() {
    assert!(is_gguf_filename(".gguf"));
}

// --- is_safetensors_filename edge cases ---

#[test]
fn test_is_safetensors_filename_mixed_case() {
    assert!(is_safetensors_filename("model.SafeTensors"));
}

#[test]
fn test_is_safetensors_filename_dot_safetensors_only() {
    assert!(is_safetensors_filename(".safetensors"));
}

// --- is_safetensors_index_filename edge cases ---

#[test]
fn test_is_safetensors_index_filename_mixed_case() {
    assert!(is_safetensors_index_filename(
        "model.SafeTensors.Index.Json"
    ));
}

// --- infer_safetensors_index_from_shard edge cases ---

#[test]
fn test_infer_index_from_shard_empty_prefix() {
    // "-00001-of-00003.safetensors" -> prefix would be empty
    let result = infer_safetensors_index_from_shard("-00001-of-00003.safetensors");
    assert!(result.is_none());
}

#[test]
fn test_infer_index_from_shard_non_numeric_shard() {
    let result = infer_safetensors_index_from_shard("model-abc-of-00003.safetensors");
    assert!(result.is_none());
}

#[test]
fn test_infer_index_from_shard_non_numeric_total() {
    let result = infer_safetensors_index_from_shard("model-00001-of-abc.safetensors");
    assert!(result.is_none());
}

// --- validate_artifact_path edge cases ---

#[test]
fn test_validate_artifact_path_nested_path() {
    assert!(validate_artifact_path("dir1/dir2/dir3/model.gguf").is_ok());
}

#[test]
fn test_validate_artifact_path_backslash_leading() {
    assert!(validate_artifact_path("\\model.gguf").is_err());
}

#[test]
fn test_validate_artifact_path_dotdot_in_middle() {
    assert!(validate_artifact_path("a/../b").is_err());
}

// --- sibling_size_bytes edge case ---

#[test]
fn test_sibling_size_bytes_lfs_none_size() {
    let s = HfSibling {
        rfilename: "model.gguf".to_string(),
        size: None,
        lfs: Some(HfLfs { size: None }),
    };
    assert_eq!(sibling_size_bytes(&s), 0);
}

// --- resolve_primary_artifact safetensors tests ---

#[test]
fn test_resolve_primary_artifact_safetensors_with_index() {
    let siblings = vec![
        HfSibling {
            rfilename: "config.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "tokenizer.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model.safetensors.index.json".to_string(),
            size: None,
            lfs: None,
        },
        HfSibling {
            rfilename: "model-00001-of-00002.safetensors".to_string(),
            size: Some(2000),
            lfs: None,
        },
        HfSibling {
            rfilename: "model-00002-of-00002.safetensors".to_string(),
            size: Some(2000),
            lfs: None,
        },
    ];
    let sel = resolve_primary_artifact(&siblings, None).unwrap();
    assert_eq!(sel.format, ArtifactFormat::Safetensors);
    assert_eq!(sel.filename, "model.safetensors.index.json");
}

#[test]
fn test_resolve_primary_artifact_empty_siblings() {
    let siblings: Vec<HfSibling> = vec![];
    assert!(resolve_primary_artifact(&siblings, None).is_err());
}
