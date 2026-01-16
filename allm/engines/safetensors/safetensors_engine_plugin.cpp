/**
 * @file safetensors_engine_plugin.cpp
 * @brief Plugin entry points for SafetensorsEngine
 *
 * SPEC-69549000: safetensors.cpp Node integration
 */

#include "safetensors_engine.h"

#include "core/engine_plugin_api.h"

namespace {
constexpr const char* kPluginId = "safetensors_cpp";
}

extern "C" {

/**
 * @brief Create a new SafetensorsEngine instance
 * @param context Engine host context
 * @return Pointer to the created engine, or nullptr on failure
 */
llm_node::Engine* llm_node_create_engine(
    const llm_node::EngineHostContext* context) {
    if (!context) {
        return nullptr;
    }

    // ABI version check
    if (context->abi_version != llm_node::kEnginePluginAbiVersion) {
        if (context->log_callback) {
            context->log_callback(
                context->log_callback_ctx,
                kPluginId,
                static_cast<int>(llm_node::PluginLogLevel::kError),
                "ABI version mismatch");
        }
        return nullptr;
    }

    // models_dir is required
    if (!context->models_dir) {
        if (context->log_callback) {
            context->log_callback(
                context->log_callback_ctx,
                kPluginId,
                static_cast<int>(llm_node::PluginLogLevel::kError),
                "models_dir is required");
        }
        return nullptr;
    }

    try {
        return new llm_node::SafetensorsEngine(context->models_dir);
    } catch (const std::exception& e) {
        if (context->log_callback) {
            std::string msg = std::string("Failed to create SafetensorsEngine: ") + e.what();
            context->log_callback(
                context->log_callback_ctx,
                kPluginId,
                static_cast<int>(llm_node::PluginLogLevel::kError),
                msg.c_str());
        }
        return nullptr;
    }
}

/**
 * @brief Destroy a SafetensorsEngine instance
 * @param engine Engine to destroy
 */
void llm_node_destroy_engine(llm_node::Engine* engine) {
    delete engine;
}

}  // extern "C"
