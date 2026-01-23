#include "models/model_registry.h"
#include "models/supported_models_json.h"
#include <algorithm>
#include <unordered_set>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

namespace xllm {

void ModelRegistry::setModels(std::vector<std::string> models) {
    std::lock_guard<std::mutex> lock(mutex_);
    models_ = std::move(models);
}

std::vector<std::string> ModelRegistry::listModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return models_;
}

std::vector<std::string> ModelRegistry::listExecutableModels() const {
    // Return all models that were added via setModels().
    // The filtering by engine.isModelSupported() already happens in main.cpp
    // when models are scanned, so no additional filtering is needed here.
    // The supportedModelMap is used for UI display purposes only.
    std::lock_guard<std::mutex> lock(mutex_);
    return models_;
}

bool ModelRegistry::hasModel(const std::string& id) const {
    std::lock_guard<std::mutex> lock(mutex_);
    return std::find(models_.begin(), models_.end(), id) != models_.end();
}

void ModelRegistry::setGpuBackend(GpuBackend backend) {
    std::lock_guard<std::mutex> lock(mutex_);
    backend_ = backend;
}

const std::unordered_map<std::string, ModelRegistry::SupportedModel>&
ModelRegistry::supportedModelMap() {
    static const std::unordered_map<std::string, SupportedModel> cached = [] {
        std::unordered_map<std::string, SupportedModel> result;
        try {
            auto root = nlohmann::json::parse(kSupportedModelsJson);
            if (!root.is_array()) {
                spdlog::error("supported_models_json is not an array");
                return result;
            }
            for (const auto& entry : root) {
                if (!entry.is_object()) {
                    continue;
                }
                const std::string id = entry.value("id", "");
                if (id.empty()) {
                    continue;
                }
                SupportedModel model{ id, {} };
                if (!entry.contains("platforms") || !entry["platforms"].is_array()) {
                    spdlog::warn("supported_models missing platforms for model: {}", id);
                } else {
                    for (const auto& platform : entry["platforms"]) {
                        if (platform.is_string()) {
                            model.platforms.push_back(platform.get<std::string>());
                        }
                    }
                    if (model.platforms.empty()) {
                        spdlog::warn("supported_models has empty platforms for model: {}", id);
                    }
                }
                result.emplace(id, std::move(model));
            }
        } catch (const std::exception& e) {
            spdlog::error("Failed to parse supported_models_json: {}", e.what());
        }
        return result;
    }();
    return cached;
}

bool ModelRegistry::isCompatible(const SupportedModel& model, GpuBackend backend) const {
    if (model.platforms.empty()) {
        return false;
    }

    const auto& platforms = model.platforms;
    auto has_platform = [&platforms](const char* value) {
        return std::find(platforms.begin(), platforms.end(), value) != platforms.end();
    };

    switch (backend) {
        case GpuBackend::Metal:
            return has_platform("macos-metal");
        case GpuBackend::Cuda:
            return has_platform("linux-cuda") || has_platform("windows-cuda");
        case GpuBackend::DirectML:
            return has_platform("windows-directml");
        case GpuBackend::Rocm:
            return has_platform("linux-rocm");
        case GpuBackend::Cpu:
            return has_platform("cpu");
    }
    return false;
}

}  // namespace xllm
