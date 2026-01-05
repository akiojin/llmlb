#pragma once

#include "core/engine.h"

namespace llm_node {

class LlamaManager;

constexpr int kEnginePluginAbiVersion = 2;

// T183: プラグインログレベル
enum class PluginLogLevel : int {
    kTrace = 0,
    kDebug = 1,
    kInfo = 2,
    kWarn = 3,
    kError = 4
};

// T183: プラグインログコールバック型
// plugin_id: ログを送信するプラグインのID
// level: ログレベル (PluginLogLevel)
// message: ログメッセージ
using PluginLogCallback = void (*)(void* ctx, const char* plugin_id, int level, const char* message);

struct EngineHostContext {
    int abi_version{0};
    const char* models_dir{nullptr};
    LlamaManager* llama_manager{nullptr};
    /// T183: プラグインログコールバック
    PluginLogCallback log_callback{nullptr};
    /// T183: ログコールバックのコンテキスト
    void* log_callback_ctx{nullptr};
};

}  // namespace llm_node

extern "C" {
    using llm_node_create_engine_fn = llm_node::Engine* (*)(const llm_node::EngineHostContext*);
    using llm_node_destroy_engine_fn = void (*)(llm_node::Engine*);
}
