// SPEC-48678000: ModelResolver - Model auto-resolution
// Resolves model paths with fallback: local -> shared -> router API
#pragma once

#include <condition_variable>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_set>
#include <vector>

namespace llm_node {

// Result of model resolution
struct ModelResolveResult {
    bool success = false;
    std::string path;           // Path to the model file (empty if not found)
    std::string error_message;  // Error message if resolution failed
    bool router_attempted = false;  // True if router API download was attempted
    bool origin_attempted = false;  // True if origin download was attempted
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
    // @param router_api_key: API key for router blob download (node scope)
    ModelResolver(std::string local_path, std::string shared_path, std::string router_url,
                  std::string router_api_key = {});

    // Resolve model path
    // @param model_name: Model identifier
    // @return ModelResolveResult with path or error
    ModelResolveResult resolve(const std::string& model_name);

    // Set allowlist patterns for direct origin download (HF, etc.)
    void setOriginAllowlist(std::vector<std::string> origin_allowlist);

    // Check if a download lock exists for the given model (for duplicate prevention)
    // @param model_name: Model identifier
    // @return true if download is in progress
    bool hasDownloadLock(const std::string& model_name) const;

    // Get download timeout in milliseconds (default: 5 minutes)
    int getDownloadTimeoutMs() const;

    // Get maximum concurrent downloads (default: 1 per node)
    int getMaxConcurrentDownloads() const;

private:
    std::string local_path_;
    std::string shared_path_;
    std::string router_url_;
    std::string router_api_key_;
    std::vector<std::string> origin_allowlist_;

    int download_timeout_ms_{5 * 60 * 1000};
    int max_concurrent_downloads_{1};
    mutable std::mutex download_mutex_;
    std::condition_variable download_cv_;
    std::unordered_set<std::string> active_downloads_;

    // Check if model exists in local cache
    std::string findLocal(const std::string& model_name);

    // Check if model exists in shared path (direct reference, no copy)
    std::string findShared(const std::string& model_name);

    // Download model from router API
    std::string downloadFromRouter(const std::string& model_name, bool* origin_attempted);
    std::string downloadFromOrigin(const std::string& model_name, const std::string& url);
    std::optional<std::string> fetchOriginUrl(const std::string& model_name);
};

}  // namespace llm_node
