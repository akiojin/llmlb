use crate::types::endpoint::EndpointType;

/// Runtime-specific model alias for one endpoint type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineAlias {
    /// Endpoint type that exposes this alias.
    pub engine: EndpointType,
    /// Runtime-native model identifier.
    pub name: &'static str,
}

/// Canonical model identifier and its runtime-specific aliases.
#[derive(Debug, Clone)]
pub struct ModelMapping {
    /// Canonical model identifier exposed by `/v1/models`.
    pub canonical: &'static str,
    /// Runtime aliases that should resolve to the canonical model.
    pub aliases: &'static [EngineAlias],
}

fn model_id_eq(left: &str, right: &str) -> bool {
    left == right || left.eq_ignore_ascii_case(right)
}

/// Built-in canonical model mappings shared by Ollama and LM Studio integration.
pub static BUILTIN_MAPPINGS: &[ModelMapping] = &[
    ModelMapping {
        canonical: "openai/gpt-oss-20b",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "gpt-oss:20b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "openai/gpt-oss-20b",
            },
        ],
    },
    ModelMapping {
        canonical: "openai/gpt-oss-120b",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "gpt-oss:120b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "openai/gpt-oss-120b",
            },
        ],
    },
    ModelMapping {
        canonical: "Qwen/qwen3-coder-30b",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3-coder:30b",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3-coder:latest",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen/qwen3-coder-30b",
            },
        ],
    },
    ModelMapping {
        canonical: "Qwen/Qwen3-30B",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3:30b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen/qwen3-30b-a3b",
            },
        ],
    },
    ModelMapping {
        canonical: "qwen/qwen3-coder-next",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3-coder-next:latest",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3-coder-next",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen/qwen3-coder-next",
            },
        ],
    },
    ModelMapping {
        canonical: "meta-llama/Llama-3.3-70B-Instruct",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "llama3.3:70b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "meta/llama-3.3-70b",
            },
        ],
    },
    ModelMapping {
        canonical: "google/gemma-3-27b-it",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "gemma3:27b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "google/gemma-3-27b",
            },
        ],
    },
    ModelMapping {
        canonical: "Qwen/Qwen3.5-35B-A3B",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3.5:35b-a3b",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3.5-35b-a3b",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen3.5:latest",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen3.5-35b-a3b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen/qwen3.5-35b-a3b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "qwen/qwen3.5-35b-a3b:2",
            },
        ],
    },
    ModelMapping {
        canonical: "nvidia/nemotron-3-super-120b-a12b",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "nemotron-3-super:120b-a12b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "nvidia-nemotron-3-super-120b-a12b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "nvidia/nemotron-3-super",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "unsloth/nvidia-nemotron-3-super-120b-a12b",
            },
        ],
    },
    ModelMapping {
        canonical: "nvidia/Nemotron-3-Nano",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "nemotron-3-nano:30b",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "nvidia/nemotron-3-nano",
            },
        ],
    },
    ModelMapping {
        canonical: "Qwen/Qwen2.5-14B-Instruct-AWQ",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "qwen2.5:14b-instruct",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "Qwen/Qwen2.5-14B-Instruct-AWQ",
            },
        ],
    },
    ModelMapping {
        canonical: "nomic-ai/nomic-embed-text-v1.5",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "nomic-embed-text:latest",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "nomic-embed-text",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "nomic-embed-text:v1.5",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "text-embedding-nomic-embed-text-v1.5",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "nomic-ai/nomic-embed-text-v1.5",
            },
        ],
    },
    ModelMapping {
        canonical: "THUDM/glm-4.7-flash",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "glm-4.7-flash:latest",
            },
            EngineAlias {
                engine: EndpointType::Ollama,
                name: "glm-4.7-flash",
            },
            EngineAlias {
                engine: EndpointType::LmStudio,
                name: "zai-org/glm-4.7-flash",
            },
        ],
    },
];

/// Resolve a runtime-native model identifier to its canonical model identifier.
pub fn resolve_canonical(model_id: &str, endpoint_type: &EndpointType) -> Option<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        if model_id_eq(mapping.canonical, model_id) {
            return Some(mapping.canonical);
        }
        for alias in mapping.aliases {
            if alias.engine == *endpoint_type && model_id_eq(alias.name, model_id) {
                return Some(mapping.canonical);
            }
        }
    }
    None
}

/// Resolve a canonical model identifier to the runtime-native alias for one endpoint type.
pub fn resolve_engine_name(canonical: &str, endpoint_type: &EndpointType) -> Option<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        if model_id_eq(mapping.canonical, canonical) {
            for alias in mapping.aliases {
                if alias.engine == *endpoint_type {
                    return Some(alias.name);
                }
            }
            return None;
        }
    }
    None
}

/// Find the built-in mapping that matches a canonical model identifier or alias.
pub fn find_mapping(model_id: &str) -> Option<&'static ModelMapping> {
    for mapping in BUILTIN_MAPPINGS {
        if model_id_eq(mapping.canonical, model_id) {
            return Some(mapping);
        }
        for alias in mapping.aliases {
            if model_id_eq(alias.name, model_id) {
                return Some(mapping);
            }
        }
    }
    None
}

/// Guess the upstream Hugging Face repository name for a runtime model identifier.
pub fn guess_hf_repo(model_id: &str, endpoint_type: &EndpointType) -> Option<String> {
    match endpoint_type {
        EndpointType::LmStudio => {
            if model_id.contains('/') && !model_id.contains(':') {
                Some(model_id.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Resolve a canonical model identifier without requiring endpoint type information.
pub fn resolve_canonical_any(model_id: &str) -> Option<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        if model_id_eq(mapping.canonical, model_id) {
            return Some(mapping.canonical);
        }
        for alias in mapping.aliases {
            if model_id_eq(alias.name, model_id) {
                return Some(mapping.canonical);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_canonical_by_ollama_name() {
        let result = resolve_canonical("gpt-oss:20b", &EndpointType::Ollama);
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_canonical_by_lm_studio_name() {
        let result = resolve_canonical("openai/gpt-oss-20b", &EndpointType::LmStudio);
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_canonical_by_canonical_name() {
        let result = resolve_canonical("openai/gpt-oss-20b", &EndpointType::Ollama);
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_canonical_unknown() {
        let result = resolve_canonical("unknown-model", &EndpointType::Ollama);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_canonical_wrong_engine() {
        let result = resolve_canonical("gpt-oss:20b", &EndpointType::Vllm);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_engine_name_ollama() {
        let result = resolve_engine_name("openai/gpt-oss-20b", &EndpointType::Ollama);
        assert_eq!(result, Some("gpt-oss:20b"));
    }

    #[test]
    fn test_resolve_engine_name_lm_studio() {
        let result = resolve_engine_name("openai/gpt-oss-20b", &EndpointType::LmStudio);
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_engine_name_unknown_canonical() {
        let result = resolve_engine_name("unknown/model", &EndpointType::Ollama);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_mapping_by_canonical() {
        let mapping = find_mapping("openai/gpt-oss-20b");
        assert!(mapping.is_some());
        let m = mapping.unwrap();
        assert_eq!(m.canonical, "openai/gpt-oss-20b");
        assert!(!m.aliases.is_empty());
    }

    #[test]
    fn test_find_mapping_by_alias() {
        let mapping = find_mapping("gpt-oss:20b");
        assert!(mapping.is_some());
        assert_eq!(mapping.unwrap().canonical, "openai/gpt-oss-20b");
    }

    #[test]
    fn test_find_mapping_not_found() {
        let mapping = find_mapping("nonexistent-model");
        assert!(mapping.is_none());
    }

    #[test]
    fn test_guess_hf_repo_lm_studio() {
        let result = guess_hf_repo(
            "lmstudio-community/gemma-3-1b-it-GGUF",
            &EndpointType::LmStudio,
        );
        assert_eq!(
            result,
            Some("lmstudio-community/gemma-3-1b-it-GGUF".to_string())
        );
    }

    #[test]
    fn test_guess_hf_repo_lm_studio_no_slash() {
        let result = guess_hf_repo("gemma-3-1b", &EndpointType::LmStudio);
        assert!(result.is_none());
    }

    #[test]
    fn test_guess_hf_repo_ollama() {
        let result = guess_hf_repo("gemma3:27b", &EndpointType::Ollama);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_canonical_any_by_canonical() {
        let result = resolve_canonical_any("openai/gpt-oss-20b");
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_canonical_any_by_alias() {
        let result = resolve_canonical_any("gpt-oss:20b");
        assert_eq!(result, Some("openai/gpt-oss-20b"));
    }

    #[test]
    fn test_resolve_canonical_any_unknown() {
        let result = resolve_canonical_any("unknown-model");
        assert!(result.is_none());
    }

    #[test]
    fn test_builtin_mappings_not_empty() {
        assert!(!BUILTIN_MAPPINGS.is_empty());
    }

    #[test]
    fn test_all_mappings_have_aliases() {
        for mapping in BUILTIN_MAPPINGS {
            assert!(
                !mapping.aliases.is_empty(),
                "Mapping for {} has no aliases",
                mapping.canonical
            );
        }
    }

    #[test]
    fn test_qwen3_coder_mapping() {
        let result = resolve_canonical("qwen3-coder:30b", &EndpointType::Ollama);
        assert_eq!(result, Some("Qwen/qwen3-coder-30b"));
    }

    #[test]
    fn test_qwen3_coder_lm_studio_lowercase_mapping() {
        let result = resolve_canonical("qwen/qwen3-coder-30b", &EndpointType::LmStudio);
        assert_eq!(result, Some("Qwen/qwen3-coder-30b"));
    }

    #[test]
    fn test_qwen3_coder_latest_mapping() {
        let result = resolve_canonical("qwen3-coder:latest", &EndpointType::Ollama);
        assert_eq!(result, Some("Qwen/qwen3-coder-30b"));
    }

    #[test]
    fn test_qwen3_coder_next_mapping() {
        let result = resolve_canonical("qwen/qwen3-coder-next", &EndpointType::LmStudio);
        assert_eq!(result, Some("qwen/qwen3-coder-next"));
    }

    #[test]
    fn test_qwen35_mapping() {
        let result = resolve_canonical("qwen3.5:latest", &EndpointType::Ollama);
        assert_eq!(result, Some("Qwen/Qwen3.5-35B-A3B"));
    }

    #[test]
    fn test_glm47_mapping() {
        let result = resolve_canonical("zai-org/glm-4.7-flash", &EndpointType::LmStudio);
        assert_eq!(result, Some("THUDM/glm-4.7-flash"));
    }

    #[test]
    fn test_nomic_embedding_mapping() {
        let result = resolve_canonical(
            "text-embedding-nomic-embed-text-v1.5",
            &EndpointType::LmStudio,
        );
        assert_eq!(result, Some("nomic-ai/nomic-embed-text-v1.5"));
    }

    #[test]
    fn test_nemotron_super_unsloth_mapping() {
        let result = resolve_canonical(
            "unsloth/nvidia-nemotron-3-super-120b-a12b",
            &EndpointType::LmStudio,
        );
        assert_eq!(result, Some("nvidia/nemotron-3-super-120b-a12b"));
    }

    #[test]
    fn test_gemma3_mapping() {
        let result = resolve_canonical("gemma3:27b", &EndpointType::Ollama);
        assert_eq!(result, Some("google/gemma-3-27b-it"));
    }

    #[test]
    fn test_llama33_mapping() {
        let result = resolve_canonical("llama3.3:70b", &EndpointType::Ollama);
        assert_eq!(result, Some("meta-llama/Llama-3.3-70B-Instruct"));
    }
}
