#pragma once

#include <string>
#include <vector>

namespace llm_node {

struct EnginePluginManifest {
    std::string engine_id;
    std::string engine_version;
    int abi_version{0};
    std::vector<std::string> runtimes;
    std::vector<std::string> formats;
    std::vector<std::string> capabilities;
    std::vector<std::string> gpu_targets;
};

class EngineHost {
public:
    static constexpr int kAbiVersion = 1;

    EngineHost() = default;
    ~EngineHost() = default;

    bool validateManifest(const EnginePluginManifest& manifest, std::string& error) const;
};

}  // namespace llm_node
