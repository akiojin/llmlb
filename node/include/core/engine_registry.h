#pragma once

#include <memory>
#include <string>
#include <unordered_map>

#include "core/engine.h"

namespace llm_node {

struct EngineDeleter {
    using DestroyFn = void (*)(Engine*);
    DestroyFn destroy{nullptr};

    void operator()(Engine* engine) const {
        if (!engine) return;
        if (destroy) {
            destroy(engine);
        } else {
            delete engine;
        }
    }
};

class EngineRegistry {
public:
    using EngineHandle = std::unique_ptr<Engine, EngineDeleter>;

    void registerEngine(std::unique_ptr<Engine> engine);
    void registerEngine(EngineHandle engine);
    Engine* resolve(const std::string& runtime) const;

private:
    std::unordered_map<std::string, EngineHandle> engines_;
};

}  // namespace llm_node
