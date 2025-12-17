// SPEC-dcaeaec4: ModelStorage - Simple model file management
// Replaces legacy compat layer with simpler directory structure:
// ~/.llm-router/models/<model_name>/model.onnx (preferred) or model.gguf (legacy)
#pragma once

#include <string>
#include <vector>
#include <optional>
#include <nlohmann/json.hpp>

namespace llm_node {

struct ModelInfo {
    // Model name (router-facing ID, typically filename-based, e.g. "gpt-oss-20b")
    std::string name;
    std::string onnx_path;  // Full path to model.onnx (if present)
    std::string gguf_path;  // Full path to model.gguf (legacy)
    bool valid{false};      // Whether the model file exists and is valid
};

class ModelStorage {
public:
    explicit ModelStorage(std::string models_dir);

    // Convert model name to directory name.
    // - Backward compatibility: replace ':' with '_'
    // - If the model id has no ':' and no '-', append "_latest" (router compatibility)
    static std::string modelNameToDir(const std::string& model_name);

    // Reverse conversion: directory name to model name.
    // NOTE: This is intentionally lossy; directory names with '_' are treated as-is.
    static std::string dirNameToModel(const std::string& dir_name);

    // FR-3: Resolve GGUF file path for a model
    // Returns empty string if model not found
    std::string resolveGguf(const std::string& model_name) const;

    // Resolve ONNX file path for a model (preferred for text/embedding/TTS)
    // Returns empty string if model not found
    std::string resolveOnnx(const std::string& model_name) const;

    // FR-4: List all available models
    std::vector<ModelInfo> listAvailable() const;

    // FR-5: Load optional metadata from metadata.json
    std::optional<nlohmann::json> loadMetadata(const std::string& model_name) const;

    // Validate model (check if model.gguf exists)
    bool validateModel(const std::string& model_name) const;

    // Delete model directory and all files
    bool deleteModel(const std::string& model_name);

private:
    std::string models_dir_;
};


}  // namespace llm_node
