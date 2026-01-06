// SPEC-dcaeaec4: ModelStorage - Simple model file management
// Replaces legacy compat layer with simpler directory structure:
// ~/.llm-router/models/<model_name>/model.gguf
#pragma once

#include <string>
#include <vector>
#include <optional>
#include <nlohmann/json.hpp>

#include "models/model_descriptor.h"

namespace llm_node {

struct ModelInfo {
    std::string name;          // Model name (e.g., "gpt-oss-20b")
    std::string format;        // "gguf" or "safetensors"
    std::string primary_path;  // Full path to primary artifact (model.gguf or safetensors/index)
    bool valid{false};      // Whether the model file exists and is valid
};

struct ParsedModelName {
    std::string base;
    std::optional<std::string> quantization;
};

class ModelStorage {
public:
    explicit ModelStorage(std::string models_dir);

    // FR-2: Convert model name to directory name (sanitized, lowercase)
    static std::string modelNameToDir(const std::string& model_name);

    // Reverse conversion: directory name to model name (best-effort)
    static std::string dirNameToModel(const std::string& dir_name);

    // Parse optional quantization suffix (modelname:quantization)
    static std::optional<ParsedModelName> parseModelName(const std::string& model_name);

    // Legacy helper: resolve GGUF file path for a model.
    // Prefer resolveDescriptor() for new code (supports safetensors too).
    std::string resolveGguf(const std::string& model_name) const;

    // FR-4: List all available models
    std::vector<ModelInfo> listAvailable() const;

    // List all available models with runtime/format metadata
    std::vector<ModelDescriptor> listAvailableDescriptors() const;

    // Resolve model descriptor (GGUF / safetensors)
    std::optional<ModelDescriptor> resolveDescriptor(const std::string& model_name) const;

    // Validate model (check if a supported artifact exists)
    bool validateModel(const std::string& model_name) const;

    // Delete model directory and all files
    bool deleteModel(const std::string& model_name);

    const std::string& modelsDir() const { return models_dir_; }

private:
    std::string models_dir_;
};


}  // namespace llm_node
