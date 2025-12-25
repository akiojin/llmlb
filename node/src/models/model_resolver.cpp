// SPEC-48678000: ModelResolver implementation
// TDD RED phase - minimal stub implementation
#include "models/model_resolver.h"

#include <filesystem>

namespace fs = std::filesystem;

namespace llm_node {

ModelResolver::ModelResolver(std::string local_path, std::string shared_path, std::string router_url)
    : local_path_(std::move(local_path)),
      shared_path_(std::move(shared_path)),
      router_url_(std::move(router_url)) {}

ModelResolveResult ModelResolver::resolve(const std::string& model_name) {
    ModelResolveResult result;

    // 1. Check local cache
    std::string local = findLocal(model_name);
    if (!local.empty()) {
        result.success = true;
        result.path = local;
        return result;
    }

    // 2. Check shared path (direct reference, no copy)
    std::string shared = findShared(model_name);
    if (!shared.empty()) {
        result.success = true;
        result.path = shared;
        return result;
    }

    // 3. Try router API download
    if (!router_url_.empty()) {
        result.router_attempted = true;
        std::string downloaded = downloadFromRouter(model_name);
        if (!downloaded.empty()) {
            result.success = true;
            result.path = downloaded;
            return result;
        }
    }

    // 4. Model not found
    result.success = false;
    result.error_message = "Model '" + model_name + "' not found in local, shared, or router";
    return result;
}

std::string ModelResolver::findLocal(const std::string& model_name) {
    if (local_path_.empty()) return "";

    fs::path model_dir = fs::path(local_path_) / model_name;
    fs::path model_file = model_dir / "model.gguf";

    if (fs::exists(model_file)) {
        return model_file.string();
    }
    return "";
}

std::string ModelResolver::findShared(const std::string& model_name) {
    if (shared_path_.empty()) return "";

    fs::path model_dir = fs::path(shared_path_) / model_name;
    fs::path model_file = model_dir / "model.gguf";

    if (fs::exists(model_file)) {
        // Direct reference - no copy to local
        return model_file.string();
    }
    return "";
}

std::string ModelResolver::downloadFromRouter(const std::string& model_name) {
    // TODO: Implement router API download (T008-T010)
    // For now, return empty (not implemented)
    (void)model_name;
    return "";
}

bool ModelResolver::hasDownloadLock(const std::string& model_name) const {
    // TODO: Implement download lock mechanism (T013)
    // For now, return false (not implemented)
    (void)model_name;
    return false;
}

}  // namespace llm_node
