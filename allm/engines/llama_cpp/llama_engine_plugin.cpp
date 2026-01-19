#include "core/engine_plugin_api.h"
#include "core/llama_engine.h"
#include "core/llama_manager.h"

extern "C" llm_node::Engine* llm_node_create_engine(const llm_node::EngineHostContext* context) {
    if (!context || context->abi_version != llm_node::kEnginePluginAbiVersion) {
        return nullptr;
    }
    if (!context->llama_manager) {
        return nullptr;
    }
    return new llm_node::LlamaEngine(*context->llama_manager);
}

extern "C" void llm_node_destroy_engine(llm_node::Engine* engine) {
    delete engine;
}
