//! モデルcapabilities自動判定
//!
//! モデル名プレフィックスからcapabilities（chat, embeddings）を自動判定

/// モデルが持つ能力
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    /// チャット/テキスト生成
    Chat,
    /// 埋め込みベクトル生成
    Embeddings,
}

impl Capability {
    /// 文字列表現を取得
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Embeddings => "embeddings",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// モデル名からcapabilitiesを自動判定
///
/// # ルール
/// - `embed*` または `*-embed*` → embeddings
/// - それ以外 → chat
///
/// # Examples
///
/// ```
/// use llmlb::sync::capabilities::{detect_capabilities, Capability};
///
/// let caps = detect_capabilities("nomic-embed-text-v1.5");
/// assert_eq!(caps, vec![Capability::Embeddings]);
///
/// let caps = detect_capabilities("llama3.2");
/// assert_eq!(caps, vec![Capability::Chat]);
/// ```
pub fn detect_capabilities(model_name: &str) -> Vec<Capability> {
    let lower = model_name.to_lowercase();

    // embedで始まる、または-embedを含む場合はembeddings
    if lower.starts_with("embed") || lower.contains("-embed") || lower.contains("_embed") {
        vec![Capability::Embeddings]
    } else {
        vec![Capability::Chat]
    }
}

/// capabilitiesをJSON用の文字列Vecに変換
pub fn capabilities_to_strings(capabilities: &[Capability]) -> Vec<String> {
    capabilities
        .iter()
        .map(|c| c.as_str().to_string())
        .collect()
}

/// 文字列からCapabilityに変換
pub fn capability_from_str(s: &str) -> Option<Capability> {
    match s.to_lowercase().as_str() {
        "chat" => Some(Capability::Chat),
        "embeddings" => Some(Capability::Embeddings),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_capabilities_embed_prefix() {
        // embedで始まるモデル
        assert_eq!(
            detect_capabilities("embed-text-v1"),
            vec![Capability::Embeddings]
        );
        assert_eq!(
            detect_capabilities("EMBED-multilingual"),
            vec![Capability::Embeddings]
        );
    }

    #[test]
    fn test_detect_capabilities_embed_suffix() {
        // -embedを含むモデル
        assert_eq!(
            detect_capabilities("nomic-embed-text-v1.5"),
            vec![Capability::Embeddings]
        );
        assert_eq!(
            detect_capabilities("bge-embed-large"),
            vec![Capability::Embeddings]
        );
        assert_eq!(
            detect_capabilities("model_embed_v2"),
            vec![Capability::Embeddings]
        );
    }

    #[test]
    fn test_detect_capabilities_chat() {
        // 通常のチャットモデル
        assert_eq!(detect_capabilities("llama3.2"), vec![Capability::Chat]);
        assert_eq!(detect_capabilities("gpt-4"), vec![Capability::Chat]);
        assert_eq!(
            detect_capabilities("gemma-2b-instruct"),
            vec![Capability::Chat]
        );
        assert_eq!(detect_capabilities("qwen2.5"), vec![Capability::Chat]);
    }

    #[test]
    fn test_detect_capabilities_case_insensitive() {
        // 大文字小文字を区別しない
        assert_eq!(
            detect_capabilities("NOMIC-EMBED-TEXT"),
            vec![Capability::Embeddings]
        );
        assert_eq!(
            detect_capabilities("Embed-Model"),
            vec![Capability::Embeddings]
        );
    }

    #[test]
    fn test_capabilities_to_strings() {
        let caps = vec![Capability::Chat];
        assert_eq!(capabilities_to_strings(&caps), vec!["chat".to_string()]);

        let caps = vec![Capability::Embeddings];
        assert_eq!(
            capabilities_to_strings(&caps),
            vec!["embeddings".to_string()]
        );
    }

    #[test]
    fn test_capability_from_str() {
        assert_eq!(capability_from_str("chat"), Some(Capability::Chat));
        assert_eq!(capability_from_str("CHAT"), Some(Capability::Chat));
        assert_eq!(
            capability_from_str("embeddings"),
            Some(Capability::Embeddings)
        );
        assert_eq!(capability_from_str("unknown"), None);
    }

    #[test]
    fn test_capability_as_str() {
        assert_eq!(Capability::Chat.as_str(), "chat");
        assert_eq!(Capability::Embeddings.as_str(), "embeddings");
    }
}
