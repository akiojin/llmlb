#pragma once

#include <memory>
#include <string>
#include <unordered_map>

#include "core/engine.h"

namespace llm_node {

class EngineRegistry {
public:
    void registerEngine(std::unique_ptr<Engine> engine);
    Engine* resolve(const std::string& runtime) const;

private:
    std::unordered_map<std::string, std::unique_ptr<Engine>> engines_;
};

}  // namespace llm_node
