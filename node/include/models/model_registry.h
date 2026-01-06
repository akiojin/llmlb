#pragma once

#include <vector>
#include <string>
#include <mutex>
#include <unordered_map>

#include "system/gpu_detector.h"

namespace llm_node {

class ModelRegistry {
public:
    struct SupportedModel {
        std::string id;
        std::vector<std::string> platforms;
    };

    void setModels(std::vector<std::string> models);
    std::vector<std::string> listModels() const;
    std::vector<std::string> listExecutableModels() const;
    bool hasModel(const std::string& id) const;
    void setGpuBackend(GpuBackend backend);

private:
    static const std::unordered_map<std::string, SupportedModel>& supportedModelMap();
    bool isCompatible(const SupportedModel& model, GpuBackend backend) const;

    mutable std::mutex mutex_;
    std::vector<std::string> models_;
    GpuBackend backend_{GpuBackend::Cpu};
};

}  // namespace llm_node
