#pragma once

#include <filesystem>
#include <string>
#include <vector>

#include "core/engine_plugin_api.h"

namespace llm_node {

class EngineRegistry;

struct EnginePluginManifest {
    std::string engine_id;
    std::string engine_version;
    int abi_version{0};
    std::vector<std::string> runtimes;
    std::vector<std::string> formats;
    std::vector<std::string> architectures;
    std::vector<std::string> capabilities;
    std::vector<std::string> modalities;
    std::vector<std::string> gpu_targets;
    std::string license;
    bool supports_vision{false};
    std::string library;
};

class EngineHost {
public:
    static constexpr int kAbiVersion = kEnginePluginAbiVersion;

    EngineHost() = default;
    ~EngineHost();

    EngineHost(const EngineHost&) = delete;
    EngineHost& operator=(const EngineHost&) = delete;

    bool validateManifest(const EnginePluginManifest& manifest, std::string& error) const;
    bool loadManifest(const std::filesystem::path& manifest_path,
                      EnginePluginManifest& manifest,
                      std::string& error) const;
    bool loadPlugin(const std::filesystem::path& manifest_path,
                    EngineRegistry& registry,
                    const EngineHostContext& context,
                    std::string& error);
    bool loadPluginsFromDir(const std::filesystem::path& directory,
                            EngineRegistry& registry,
                            const EngineHostContext& context,
                            std::string& error);

private:
    struct LoadedPlugin {
        std::string engine_id;
        std::filesystem::path library_path;
        void* handle{nullptr};
    };

    std::vector<LoadedPlugin> plugins_;
};

}  // namespace llm_node
