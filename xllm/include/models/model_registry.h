#pragma once

#include <vector>
#include <string>
#include <mutex>
#include <unordered_set>

#include "system/gpu_detector.h"

namespace xllm {

class ModelRegistry {
public:
    void setModels(std::vector<std::string> models);
    std::vector<std::string> listModels() const;
    std::vector<std::string> listExecutableModels() const;
    bool hasModel(const std::string& id) const;
    void setGpuBackend(GpuBackend backend);

    // Vision capability tracking
    void setVisionCapable(const std::string& model_name, bool capable);
    bool hasVisionCapability(const std::string& model_name) const;

private:
    mutable std::mutex mutex_;
    std::vector<std::string> models_;
    std::unordered_set<std::string> vision_models_;  // Models with mmproj/vision capability
    GpuBackend backend_{GpuBackend::Cpu};
};

}  // namespace xllm
