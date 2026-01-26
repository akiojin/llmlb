#include "models/model_registry.h"
#include <algorithm>

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
    // Model architecture detection is done via config.json in ModelStorage.
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

void ModelRegistry::setVisionCapable(const std::string& model_name, bool capable) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (capable) {
        vision_models_.insert(model_name);
    } else {
        vision_models_.erase(model_name);
    }
}

bool ModelRegistry::hasVisionCapability(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(mutex_);
    return vision_models_.count(model_name) > 0;
}

}  // namespace xllm
