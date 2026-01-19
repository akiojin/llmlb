#include "core/engine_plugin_api.h"
#include "core/llama_engine.h"
#include "core/llama_manager.h"

extern "C" allm::Engine* allm_create_engine(const allm::EngineHostContext* context) {
    if (!context || context->abi_version != allm::kEnginePluginAbiVersion) {
        return nullptr;
    }
    if (!context->llama_manager) {
        return nullptr;
    }
    return new allm::LlamaEngine(*context->llama_manager);
}

extern "C" void allm_destroy_engine(allm::Engine* engine) {
    delete engine;
}
