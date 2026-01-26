// SPEC-58378000: OllamaModel data model
// Represents an ollama model reference (read-only)

#pragma once

#include <string>
#include <cstdint>

namespace xllm {
namespace cli {
namespace models {

/// Ollama model reference (read-only)
struct OllamaModel {
    /// Ollama model name (e.g., "llama3.2")
    std::string name;

    /// Path to manifest.json
    std::string manifest_path;

    /// SHA256 digest of the blob
    std::string blob_digest;

    /// Path to the blob file
    std::string blob_path;

    /// File size in bytes
    uint64_t size_bytes{0};

    /// Always true (read-only)
    static constexpr bool readonly = true;

    /// Format name for display with ollama: prefix
    std::string formatDisplayName() const;

    /// Format size for display (e.g., "4.1 GB")
    std::string formatSize() const;

    /// Check if the blob file exists
    bool blobExists() const;

    /// Check if this is a valid ollama model reference
    bool isValid() const;

    /// Get short digest for display (first 12 chars)
    std::string shortDigest() const;
};

}  // namespace models
}  // namespace cli
}  // namespace xllm
