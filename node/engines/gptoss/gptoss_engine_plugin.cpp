#include "core/engine_plugin_api.h"
#include "core/gptoss_engine.h"

extern "C" llm_node::Engine* llm_node_create_engine(const llm_node::EngineHostContext* context) {
    if (!context || context->abi_version != llm_node::kEnginePluginAbiVersion) {
        return nullptr;
    }
    return new llm_node::GptOssEngine();
}

extern "C" void llm_node_destroy_engine(llm_node::Engine* engine) {
    delete engine;
}
