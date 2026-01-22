#pragma once

#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "core/engine_registry.h"

namespace xllm {

class LlamaManager;
class Engine;

class TextManager {
public:
    TextManager(LlamaManager& llama_manager, std::string models_dir);

    Engine* resolve(const ModelDescriptor& descriptor,
                    const std::string& capability,
                    std::string* error = nullptr) const;
    Engine* resolve(const ModelDescriptor& descriptor) const;

    bool supportsArchitecture(const std::string& runtime,
                              const std::vector<std::string>& architectures) const;

    size_t engineIdCount() const;
    std::string engineIdFor(const Engine* engine) const;
    std::vector<std::string> getRegisteredRuntimes() const;

#ifdef XLLM_TESTING
    void setEngineRegistryForTest(std::unique_ptr<EngineRegistry> registry);
#endif

private:
    std::unique_ptr<EngineRegistry> registry_;
    std::string models_dir_;
};

}  // namespace xllm
