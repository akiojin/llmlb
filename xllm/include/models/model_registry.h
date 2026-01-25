#pragma once

#include <vector>
#include <string>
#include <mutex>

#include "system/gpu_detector.h"

namespace xllm {

class ModelRegistry {
public:
    void setModels(std::vector<std::string> models);
    std::vector<std::string> listModels() const;
    std::vector<std::string> listExecutableModels() const;
    bool hasModel(const std::string& id) const;
    void setGpuBackend(GpuBackend backend);

private:
    mutable std::mutex mutex_;
    std::vector<std::string> models_;
    GpuBackend backend_{GpuBackend::Cpu};
};

}  // namespace xllm
