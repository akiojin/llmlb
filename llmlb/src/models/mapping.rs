//! モデル名マッピングモジュール
//!
//! 同一モデルがエンジンごとに異なるmodel_idで登録される問題を解決するため、
//! 正規名（HFリポ名）とエンジン固有名のマッピングテーブルを提供する。

use crate::types::endpoint::EndpointType;

/// エンジン固有のモデル名エイリアス
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineAlias {
    /// エンドポイントタイプ
    pub engine: EndpointType,
    /// エンジン固有のモデル名
    pub name: &'static str,
}

/// モデルマッピング定義
#[derive(Debug, Clone)]
pub struct ModelMapping {
    /// 正規名（HFリポ名）
    pub canonical: &'static str,
    /// エンジン固有のエイリアス一覧
    pub aliases: &'static [EngineAlias],
}

/// 組み込みマッピングテーブル
///
/// 実環境で確認されたモデル名の対応関係を定義。
/// リリース時にのみ更新する。
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
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "gpt-oss:120b",
        }],
    },
    ModelMapping {
        canonical: "Qwen/qwen3-coder-30b",
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "qwen3-coder:30b",
        }],
    },
    ModelMapping {
        canonical: "Qwen/Qwen3-30B",
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "qwen3:30b",
        }],
    },
    ModelMapping {
        canonical: "meta-llama/Llama-3.3-70B-Instruct",
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "llama3.3:70b",
        }],
    },
    ModelMapping {
        canonical: "google/gemma-3-27b-it",
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "gemma3:27b",
        }],
    },
    ModelMapping {
        canonical: "Qwen/Qwen3.5-35B-A3B",
        aliases: &[
            EngineAlias {
                engine: EndpointType::Ollama,
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
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "nvidia-nemotron-3-super-120b-a12b",
        }],
    },
    ModelMapping {
        canonical: "nvidia/Nemotron-3-Nano",
        aliases: &[EngineAlias {
            engine: EndpointType::LmStudio,
            name: "nvidia/nemotron-3-nano",
        }],
    },
    // Qwen2.5-14B-Instruct-AWQ: LM Studioが正規名そのままで報告するため
    // canonical直接一致で解決されるが、find_mapping()でエイリアス検索にも
    // ヒットさせるために明示的に登録
    ModelMapping {
        canonical: "Qwen/Qwen2.5-14B-Instruct-AWQ",
        aliases: &[EngineAlias {
            engine: EndpointType::LmStudio,
            name: "Qwen/Qwen2.5-14B-Instruct-AWQ",
        }],
    },
    ModelMapping {
        canonical: "nomic-ai/nomic-embed-text-v1.5",
        aliases: &[EngineAlias {
            engine: EndpointType::Ollama,
            name: "text-embedding-nomic-embed-text-v1.5",
        }],
    },
    ModelMapping {
        canonical: "THUDM/glm-4.7-flash",
        aliases: &[EngineAlias {
            engine: EndpointType::LmStudio,
            name: "zai-org/glm-4.7-flash",
        }],
    },
];

/// モデルIDとエンドポイントタイプから正規名を解決する
///
/// マッピングテーブルを検索し、エンジン固有名から正規名を返す。
/// 正規名がそのまま渡された場合もマッチする。
pub fn resolve_canonical(model_id: &str, endpoint_type: &EndpointType) -> Option<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        // 正規名が直接渡された場合
        if mapping.canonical == model_id {
            return Some(mapping.canonical);
        }
        // エンジン固有名から正規名を解決
        for alias in mapping.aliases {
            if alias.engine == *endpoint_type && alias.name == model_id {
                return Some(mapping.canonical);
            }
        }
    }
    None
}

/// 正規名からエンジン固有名を解決する
///
/// 指定されたエンドポイントタイプに対応するエンジン固有名を返す。
pub fn resolve_engine_name(canonical: &str, endpoint_type: &EndpointType) -> Option<&'static str> {
    resolve_engine_names(canonical, endpoint_type)
        .into_iter()
        .next()
}

/// Resolve all engine-specific aliases for a canonical model on a given endpoint type.
pub fn resolve_engine_names(canonical: &str, endpoint_type: &EndpointType) -> Vec<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        if mapping.canonical == canonical {
            return mapping
                .aliases
                .iter()
                .filter(|alias| alias.engine == *endpoint_type)
                .map(|alias| alias.name)
                .collect();
        }
    }
    Vec::new()
}

/// モデルIDから全エイリアス情報を検索する
///
/// 正規名またはエンジン固有名のいずれかにマッチするマッピングを返す。
pub fn find_mapping(model_id: &str) -> Option<&'static ModelMapping> {
    for mapping in BUILTIN_MAPPINGS {
        if mapping.canonical == model_id {
            return Some(mapping);
        }
        for alias in mapping.aliases {
            if alias.name == model_id {
                return Some(mapping);
            }
        }
    }
    None
}

/// モデルIDからHFリポ名を推測する（マッピングテーブル外のフォールバック）
///
/// LM StudioのモデルIDは `publisher/model-name` 形式が多いため、
/// そのまま HFリポ名として扱える場合がある。
/// Ollamaのモデルは `model:tag` 形式のため推測が困難。
pub fn guess_hf_repo(model_id: &str, endpoint_type: &EndpointType) -> Option<String> {
    match endpoint_type {
        EndpointType::LmStudio => {
            // LM Studio: "publisher/model-name" 形式ならそのままHFリポ名候補
            if model_id.contains('/') && !model_id.contains(':') {
                Some(model_id.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// モデルIDが任意のエイリアスとしてマッピングに存在するか確認し、
/// 存在する場合は正規名を返す（エンドポイントタイプ不問）
pub fn resolve_canonical_any(model_id: &str) -> Option<&'static str> {
    for mapping in BUILTIN_MAPPINGS {
        if mapping.canonical == model_id {
            return Some(mapping.canonical);
        }
        for alias in mapping.aliases {
            if alias.name == model_id {
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
        // gpt-oss:20b はOllamaのエイリアスなのでvLLMでは解決できない
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
    fn test_resolve_engine_name_no_alias() {
        let result = resolve_engine_name("openai/gpt-oss-20b", &EndpointType::Vllm);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_engine_name_unknown_canonical() {
        let result = resolve_engine_name("unknown/model", &EndpointType::Ollama);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_engine_names_lm_studio_returns_all_aliases() {
        let result = resolve_engine_names("Qwen/Qwen3.5-35B-A3B", &EndpointType::LmStudio);
        assert_eq!(
            result,
            vec!["qwen/qwen3.5-35b-a3b", "qwen/qwen3.5-35b-a3b:2"]
        );
    }

    #[test]
    fn test_resolve_engine_names_unknown_canonical_returns_empty() {
        let result = resolve_engine_names("unknown/model", &EndpointType::LmStudio);
        assert!(result.is_empty());
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
    fn test_gemma3_mapping() {
        let result = resolve_canonical("gemma3:27b", &EndpointType::Ollama);
        assert_eq!(result, Some("google/gemma-3-27b-it"));
    }

    #[test]
    fn test_llama33_mapping() {
        let result = resolve_canonical("llama3.3:70b", &EndpointType::Ollama);
        assert_eq!(result, Some("meta-llama/Llama-3.3-70B-Instruct"));
    }

    #[test]
    fn test_nvidia_nemotron_super_mapping() {
        let result = resolve_canonical("nvidia-nemotron-3-super-120b-a12b", &EndpointType::Ollama);
        assert_eq!(result, Some("nvidia/nemotron-3-super-120b-a12b"));
    }

    #[test]
    fn test_nvidia_nemotron_nano_mapping() {
        let result = resolve_canonical("nvidia/nemotron-3-nano", &EndpointType::LmStudio);
        assert_eq!(result, Some("nvidia/Nemotron-3-Nano"));
    }

    #[test]
    fn test_nomic_embed_mapping() {
        let result = resolve_canonical(
            "text-embedding-nomic-embed-text-v1.5",
            &EndpointType::Ollama,
        );
        assert_eq!(result, Some("nomic-ai/nomic-embed-text-v1.5"));
    }

    #[test]
    fn test_glm_flash_mapping() {
        let result = resolve_canonical("zai-org/glm-4.7-flash", &EndpointType::LmStudio);
        assert_eq!(result, Some("THUDM/glm-4.7-flash"));
    }

    #[test]
    fn test_qwen25_awq_mapping() {
        let result = resolve_canonical("Qwen/Qwen2.5-14B-Instruct-AWQ", &EndpointType::LmStudio);
        assert_eq!(result, Some("Qwen/Qwen2.5-14B-Instruct-AWQ"));
    }

    #[test]
    fn test_qwen35_all_variants_resolve_to_same_canonical() {
        let ollama = resolve_canonical("qwen3.5-35b-a3b", &EndpointType::Ollama);
        let lms = resolve_canonical("qwen/qwen3.5-35b-a3b", &EndpointType::LmStudio);
        let lms_v2 = resolve_canonical("qwen/qwen3.5-35b-a3b:2", &EndpointType::LmStudio);
        assert_eq!(ollama, lms);
        assert_eq!(lms, lms_v2);
        assert_eq!(ollama, Some("Qwen/Qwen3.5-35B-A3B"));
    }
}
