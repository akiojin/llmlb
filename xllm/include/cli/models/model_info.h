// SPEC-58378000: Model data model
// Represents a locally stored LLM model

#pragma once

#include <string>
#include <optional>
#include <chrono>

namespace xllm {
namespace cli {
namespace models {

/// Model format type
enum class ModelFormat {
    GGUF,
    Safetensors
};

/// Model source type
enum class ModelSource {
    Local,
    Ollama,
    HuggingFace
};

/// Locally stored LLM model
struct ModelInfo {
    /// Model name (e.g., "meta-llama/Llama-3.2-3B-Instruct")
    std::string name;

    /// Short alias (e.g., "llama3.2-3b")
    std::optional<std::string> alias;

    /// File path
    std::string path;

    /// Model format
    ModelFormat format{ModelFormat::GGUF};

    /// File size in bytes
    uint64_t size_bytes{0};

    /// Architecture (e.g., "llama")
    std::string architecture;

    /// Quantization (e.g., "Q4_K_M")
    std::optional<std::string> quantization;

    /// Number of parameters
    std::optional<uint64_t> parameters;

    /// Context length
    std::optional<uint32_t> context_length;

    /// Model source
    ModelSource source{ModelSource::Local};

    /// Creation timestamp
    std::chrono::system_clock::time_point created_at;

    /// Last used timestamp
    std::optional<std::chrono::system_clock::time_point> last_used_at;

    /// Check if model name is valid
    /// Valid: non-empty, alphanumeric with / - _ .
    static bool isValidName(const std::string& name);

    /// Format size for display (e.g., "6.4 GB")
    std::string formatSize() const;

    /// Format modified time for display (e.g., "2 hours ago")
    std::string formatModified() const;

    /// Get model format as string
    static std::string formatToString(ModelFormat fmt);

    /// Get model source as string
    static std::string sourceToString(ModelSource src);
};

}  // namespace models
}  // namespace cli
}  // namespace xllm
