#include "core/engine_host.h"

namespace llm_node {

bool EngineHost::validateManifest(const EnginePluginManifest&,
                                  std::string& error) const {
    error.clear();

    if (manifest.engine_id.empty()) {
        error = "engine_id is required";
        return false;
    }
    if (manifest.engine_version.empty()) {
        error = "engine_version is required";
        return false;
    }
    if (manifest.abi_version != kAbiVersion) {
        error = "abi_version mismatch";
        return false;
    }
    if (manifest.runtimes.empty()) {
        error = "runtimes is required";
        return false;
    }
    if (manifest.formats.empty()) {
        error = "formats is required";
        return false;
    }

    for (const auto& runtime : manifest.runtimes) {
        if (runtime.empty()) {
            error = "runtimes contains empty value";
            return false;
        }
    }
    for (const auto& format : manifest.formats) {
        if (format.empty()) {
            error = "formats contains empty value";
            return false;
        }
    }

    return true;
}

}  // namespace llm_node
