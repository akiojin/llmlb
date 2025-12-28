#include "core/engine_host.h"

namespace llm_node {

bool EngineHost::validateManifest(const EnginePluginManifest&,
                                  std::string& error) const {
    error = "EngineHost manifest validation not implemented";
    return false;
}

}  // namespace llm_node
