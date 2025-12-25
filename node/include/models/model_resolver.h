// SPEC-48678000: ModelResolver - Model auto-resolution
// Resolves model paths with fallback: local -> shared -> router API
#pragma once

#include <string>

namespace llm_node {

// Result of model resolution
struct ModelResolveResult {
    bool success = false;
    std::string path;          // Path to the model file (empty if not found)
    std::string error_message; // Error message if resolution failed
};

// Model resolver with fallback strategy:
// 1. Local cache
// 2. Shared path (router-side cache, direct reference without copy)
// 3. Router HTTP API download
class ModelResolver {
public:
    // Constructor
    // @param local_path: Local cache directory (e.g., ~/.llm-node/models)
    // @param shared_path: Shared storage path (router-side cache, NFS mount)
    // @param router_url: Router API base URL for model download
    ModelResolver(std::string local_path, std::string shared_path, std::string router_url);

    // Resolve model path
    // @param model_name: Model identifier
    // @return ModelResolveResult with path or error
    ModelResolveResult resolve(const std::string& model_name);

private:
    std::string local_path_;
    std::string shared_path_;
    std::string router_url_;

    // Check if model exists in local cache
    std::string findLocal(const std::string& model_name);

    // Check if model exists in shared path (direct reference, no copy)
    std::string findShared(const std::string& model_name);

    // Download model from router API
    std::string downloadFromRouter(const std::string& model_name);
};

}  // namespace llm_node
