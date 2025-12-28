#include "core/engine_registry.h"

namespace llm_node {

void EngineRegistry::registerEngine(std::unique_ptr<Engine> engine) {
    if (!engine) return;
    EngineHandle handle(engine.release(), EngineDeleter{});
    registerEngine(std::move(handle));
}

void EngineRegistry::registerEngine(EngineHandle engine) {
    if (!engine) return;
    engines_[engine->runtime()] = std::move(engine);
}

Engine* EngineRegistry::resolve(const std::string& runtime) const {
    auto it = engines_.find(runtime);
    if (it == engines_.end()) return nullptr;
    return it->second.get();
}

}  // namespace llm_node
