#pragma once

#include <string>
#include <vector>
#include <optional>
#include <cstdint>

namespace llm_node {
namespace cli {

/// Information about an ollama model from manifest
struct OllamaModelInfo {
    std::string name;           // Model name (e.g., "llama3.2")
    std::string manifest_path;  // Path to manifest.json
    std::string blob_digest;    // SHA256 digest of the model blob
    std::string blob_path;      // Full path to blob file
    uint64_t size_bytes;        // Size of the model blob
    bool readonly{true};        // Always true (read-only reference)
};

/// Ollama compatibility layer for reading ~/.ollama/models/
class OllamaCompat {
public:
    /// Constructor
    /// @param ollama_dir Path to ollama models directory (default: ~/.ollama/models)
    explicit OllamaCompat(const std::string& ollama_dir = "");

    ~OllamaCompat();

    /// Check if ollama models directory exists
    bool isAvailable() const;

    /// List all available ollama models
    std::vector<OllamaModelInfo> listModels() const;

    /// Get model info by name
    /// @param name Model name (e.g., "llama3.2", "mistral")
    /// @return Model info if found
    std::optional<OllamaModelInfo> getModel(const std::string& name) const;

    /// Resolve model blob path
    /// @param name Model name
    /// @return Full path to GGUF blob file, or empty if not found
    std::string resolveBlobPath(const std::string& name) const;

    /// Get ollama models directory
    const std::string& getOllamaDir() const { return ollama_dir_; }

    /// Check if a model name has ollama prefix
    /// @param name Model name (e.g., "ollama:llama3.2")
    /// @return true if name starts with "ollama:"
    static bool hasOllamaPrefix(const std::string& name);

    /// Strip ollama prefix from model name
    /// @param name Model name with prefix
    /// @return Model name without prefix
    static std::string stripOllamaPrefix(const std::string& name);

private:
    std::string ollama_dir_;

    /// Parse manifest.json file
    /// @param manifest_path Path to manifest.json
    /// @param model_name Model name
    /// @return Model info if parsing succeeds
    std::optional<OllamaModelInfo> parseManifest(
        const std::string& manifest_path,
        const std::string& model_name
    ) const;

    /// Find blob file by digest
    /// @param digest SHA256 digest (with or without sha256: prefix)
    /// @return Full path to blob file, or empty if not found
    std::string findBlob(const std::string& digest) const;

    /// Get default ollama models directory
    static std::string getDefaultOllamaDir();
};

}  // namespace cli
}  // namespace llm_node
