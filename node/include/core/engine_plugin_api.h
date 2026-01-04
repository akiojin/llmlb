#pragma once

#include "core/engine.h"

namespace llm_node {

class LlamaManager;

constexpr int kEnginePluginAbiVersion = 2;

struct EngineHostContext {
    int abi_version{0};
    const char* models_dir{nullptr};
    LlamaManager* llama_manager{nullptr};
};

}  // namespace llm_node

extern "C" {
    using llm_node_create_engine_fn = llm_node::Engine* (*)(const llm_node::EngineHostContext*);
    using llm_node_destroy_engine_fn = void (*)(llm_node::Engine*);
}
