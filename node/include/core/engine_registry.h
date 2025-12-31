#pragma once

#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

#include "core/engine.h"

namespace llm_node {

struct EngineRegistration {
    std::string engine_id;
    std::string engine_version;
    std::vector<std::string> formats;
    std::vector<std::string> capabilities;
};

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

    bool registerEngine(std::unique_ptr<Engine> engine, const EngineRegistration& registration, std::string* error);
    bool registerEngine(EngineHandle engine, const EngineRegistration& registration, std::string* error);
    void registerEngine(std::unique_ptr<Engine> engine);
    void registerEngine(EngineHandle engine);
    Engine* resolve(const std::string& runtime) const;
    Engine* resolve(const ModelDescriptor& descriptor) const;

private:
    struct EngineEntry {
        std::string engine_id;
        std::string engine_version;
        std::vector<std::string> formats;
        std::vector<std::string> capabilities;
        EngineHandle engine;
    };

    std::unordered_map<std::string, std::vector<EngineEntry>> engines_;
    std::unordered_map<std::string, std::string> engine_ids_;
};

}  // namespace llm_node
