// SPEC-48678000: ModelResolver implementation
// Resolves model paths with fallback: local -> router manifest (direct origin URLs)
#include "models/model_resolver.h"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <memory>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#include "models/model_downloader.h"
#include "models/model_storage.h"
#include "utils/allowlist.h"

namespace fs = std::filesystem;

namespace llm_node {

namespace {
bool isMetalArtifactName(const std::string& name) {
    const auto lower = toLowerAscii(name);
    return lower == "model.metal.bin";
}

bool isDirectMlArtifactName(const std::string& name) {
    const auto lower = toLowerAscii(name);
    return lower == "model.directml.bin" || lower == "model.dml.bin";
}

bool shouldDownloadForBackend(const std::string& name) {
#if defined(__APPLE__)
    if (isDirectMlArtifactName(name)) return false;
    return true;
#elif defined(_WIN32)
    if (isMetalArtifactName(name)) return false;
    return true;
#else
    if (isMetalArtifactName(name) || isDirectMlArtifactName(name)) return false;
    return true;
#endif
}
}  // namespace

ModelResolver::ModelResolver(std::string local_path, std::string router_url, std::string router_api_key)
    : local_path_(std::move(local_path)),
      router_url_(std::move(router_url)),
      router_api_key_(std::move(router_api_key)) {}

ModelResolveResult ModelResolver::resolve(const std::string& model_name) {
    ModelResolveResult result;

    // 1. Check local cache
    std::string local = findLocal(model_name);
    if (!local.empty()) {
        result.success = true;
        result.path = local;
        return result;
    }

    if (router_url_.empty()) {
        result.success = false;
        result.error_message = "Model '" + model_name + "' not found locally and router_url is not configured";
        return result;
    }

    // 2. Download via router manifest (direct origin URLs)
    result.router_attempted = true;
    std::string downloaded = downloadFromRegistry(model_name);
    if (!downloaded.empty()) {
        result.success = true;
        result.path = downloaded;
        return result;
    }

    // 3. Model not resolved
    result.success = false;
    result.error_message = "Model '" + model_name +
        "' could not be resolved from local cache or registry";
    return result;
}

void ModelResolver::setOriginAllowlist(std::vector<std::string> origin_allowlist) {
    origin_allowlist_ = std::move(origin_allowlist);
}

std::string ModelResolver::findLocal(const std::string& model_name) {
    if (local_path_.empty()) return "";

    ModelStorage storage(local_path_);
    auto desc = storage.resolveDescriptor(model_name);
    if (desc && !desc->primary_path.empty()) {
        return desc->primary_path;
    }
    return "";
}

std::string ModelResolver::downloadFromRegistry(const std::string& model_name) {
    if (router_url_.empty() || local_path_.empty()) return "";

    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(download_timeout_ms_);
    {
        std::unique_lock<std::mutex> lock(download_mutex_);
        while (active_downloads_.count(model_name) > 0 ||
               static_cast<int>(active_downloads_.size()) >= max_concurrent_downloads_) {
            if (download_cv_.wait_until(lock, deadline) == std::cv_status::timeout) {
                lock.unlock();
                return findLocal(model_name);
            }
        }
        active_downloads_.insert(model_name);
    }

    auto release = [this, model_name](void*) {
        std::lock_guard<std::mutex> lock(download_mutex_);
        active_downloads_.erase(model_name);
        download_cv_.notify_all();
    };
    auto release_guard = std::unique_ptr<void, decltype(release)>(nullptr, release);

    // Re-check local after acquiring slot (another download may have finished)
    if (auto local = findLocal(model_name); !local.empty()) {
        return local;
    }

    std::string registry_base = router_url_;
    if (!registry_base.empty() && registry_base.back() == '/') {
        registry_base.pop_back();
    }
    registry_base += "/v0/models/registry";

    ModelDownloader downloader(
        registry_base,
        local_path_,
        std::chrono::milliseconds(download_timeout_ms_),
        2,
        std::chrono::milliseconds(200),
        router_api_key_);

    auto manifest_path = downloader.fetchManifest(model_name);
    if (manifest_path.empty()) {
        return "";
    }

    std::ifstream ifs(manifest_path);
    nlohmann::json manifest = nlohmann::json::parse(ifs, nullptr, false);
    if (manifest.is_discarded() || !manifest.contains("files") || !manifest["files"].is_array()) {
        return "";
    }

    for (const auto& f : manifest["files"]) {
        std::string name = f.value("name", "");
        if (name.empty()) return "";
        if (!shouldDownloadForBackend(name)) continue;

        std::string url = f.value("url", "");
        if (url.empty()) {
            spdlog::warn("ModelResolver: manifest missing url for model {} file {}", model_name, name);
            return "";
        }
        if (!isUrlAllowedByAllowlist(url, origin_allowlist_)) {
            spdlog::warn("ModelResolver: origin URL blocked by allowlist for model {} file {}", model_name, name);
            return "";
        }

        const std::string rel_path = ModelStorage::modelNameToDir(model_name) + "/" + name;
        const auto downloaded = downloader.downloadBlob(url, rel_path, nullptr);
        if (downloaded.empty()) {
            return "";
        }
    }

    return findLocal(model_name);
}

bool ModelResolver::hasDownloadLock(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(download_mutex_);
    return active_downloads_.count(model_name) > 0;
}

int ModelResolver::getDownloadTimeoutMs() const {
    return download_timeout_ms_;
}

int ModelResolver::getMaxConcurrentDownloads() const {
    return max_concurrent_downloads_;
}

}  // namespace llm_node
