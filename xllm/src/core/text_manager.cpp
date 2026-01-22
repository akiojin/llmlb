#include "core/text_manager.h"

#include "core/llama_engine.h"
#include "core/llama_manager.h"

#ifdef XLLM_WITH_SAFETENSORS
#include "safetensors_engine.h"
#endif

namespace xllm {

TextManager::TextManager(LlamaManager& llama_manager, std::string models_dir)
    : registry_(std::make_unique<EngineRegistry>())
    , models_dir_(std::move(models_dir)) {
    EngineRegistration llama_reg;
    llama_reg.engine_id = "llama_cpp";
    llama_reg.engine_version = "builtin";
    llama_reg.formats = {"gguf"};
    llama_reg.architectures = {"llama", "mistral", "gemma", "phi"};
    llama_reg.capabilities = {"text", "embeddings"};
    registry_->registerEngine(std::make_unique<LlamaEngine>(llama_manager), llama_reg, nullptr);

#ifdef XLLM_WITH_SAFETENSORS
    EngineRegistration safetensors_reg;
    safetensors_reg.engine_id = "safetensors_cpp";
    safetensors_reg.engine_version = "builtin";
    safetensors_reg.formats = {"safetensors"};
    safetensors_reg.architectures = {
        "llama",
        "mistral",
        "gemma",
        "qwen",
        "phi",
        "nemotron",
        "deepseek",
        "gptoss",
        "granite",
        "smollm",
        "kimi",
        "moondream",
        "devstral",
        "magistral",
        "snowflake",
        "nomic",
        "mxbai",
        "minilm"};
    safetensors_reg.capabilities = {"text", "embeddings"};
    registry_->registerEngine(std::make_unique<SafetensorsEngine>(models_dir_), safetensors_reg, nullptr);
#endif
}

Engine* TextManager::resolve(const ModelDescriptor& descriptor,
                             const std::string& capability,
                             std::string* error) const {
    if (!registry_) {
        if (error) {
            *error = "TextManager not initialized";
        }
        return nullptr;
    }
    return registry_->resolve(descriptor, capability, error);
}

Engine* TextManager::resolve(const ModelDescriptor& descriptor) const {
    if (!registry_) {
        return nullptr;
    }
    return registry_->resolve(descriptor);
}

bool TextManager::supportsArchitecture(const std::string& runtime,
                                       const std::vector<std::string>& architectures) const {
    if (!registry_) {
        return false;
    }
    return registry_->supportsArchitecture(runtime, architectures);
}

size_t TextManager::engineIdCount() const {
    return registry_ ? registry_->engineIdCount() : 0;
}

std::string TextManager::engineIdFor(const Engine* engine) const {
    return registry_ ? registry_->engineIdFor(engine) : std::string();
}

std::vector<std::string> TextManager::getRegisteredRuntimes() const {
    return registry_ ? registry_->getRegisteredRuntimes() : std::vector<std::string>{};
}

#ifdef XLLM_TESTING
void TextManager::setEngineRegistryForTest(std::unique_ptr<EngineRegistry> registry) {
    registry_ = std::move(registry);
}
#endif

}  // namespace xllm
