// SPEC-48678000: ModelResolver implementation
// Resolves model paths with fallback: local -> shared -> router API download
#include "models/model_resolver.h"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <memory>
#include <thread>

#include <spdlog/spdlog.h>

#include "models/model_downloader.h"
#include "models/model_storage.h"

namespace fs = std::filesystem;

namespace llm_node {

namespace {
bool isRegularFile(const fs::path& path) {
    std::error_code ec;
    auto st = fs::symlink_status(path, ec);
    if (ec) return false;
    return st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
}

std::string urlEncodePathSegment(const std::string& input) {
    static const char* kHex = "0123456789ABCDEF";
    std::string out;
    out.reserve(input.size());
    for (unsigned char c : input) {
        const bool unreserved =
            (c >= 'A' && c <= 'Z') ||
            (c >= 'a' && c <= 'z') ||
            (c >= '0' && c <= '9') ||
            c == '-' || c == '_' || c == '.' || c == '~';
        if (unreserved) {
            out.push_back(static_cast<char>(c));
        } else {
            out.push_back('%');
            out.push_back(kHex[(c >> 4) & 0x0F]);
            out.push_back(kHex[c & 0x0F]);
        }
    }
    return out;
}

bool hasGgufMagic(const fs::path& path) {
    std::ifstream ifs(path, std::ios::binary);
    if (!ifs.is_open()) return false;
    char magic[4] = {0, 0, 0, 0};
    ifs.read(magic, sizeof(magic));
    return ifs.gcount() == 4 && magic[0] == 'G' && magic[1] == 'G' && magic[2] == 'U' && magic[3] == 'F';
}
}  // namespace

ModelResolver::ModelResolver(std::string local_path, std::string shared_path, std::string router_url,
                             std::string router_api_key)
    : local_path_(std::move(local_path)),
      shared_path_(std::move(shared_path)),
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
    if (router_url_.empty()) {
        result.error_message = "Model '" + model_name + "' not found in local or shared path";
    } else {
        result.error_message = "Model '" + model_name + "' not found in local, shared, or router";
    }
    return result;
}

std::string ModelResolver::findLocal(const std::string& model_name) {
    if (local_path_.empty()) return "";

    const auto dir_name = ModelStorage::modelNameToDir(model_name);
    fs::path model_file = fs::path(local_path_) / dir_name / "model.gguf";

    if (isRegularFile(model_file)) {
        return model_file.string();
    }
    return "";
}

std::string ModelResolver::findShared(const std::string& model_name) {
    if (shared_path_.empty()) return "";

    const auto dir_name = ModelStorage::modelNameToDir(model_name);
    fs::path model_file = fs::path(shared_path_) / dir_name / "model.gguf";

    // Use error_code overloads to avoid exceptions on NFS disconnection or permission errors
    if (isRegularFile(model_file)) {
        // Direct reference - no copy to local
        return model_file.string();
    }
    return "";
}

std::string ModelResolver::downloadFromRouter(const std::string& model_name) {
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

    const auto dir_name = ModelStorage::modelNameToDir(model_name);
    fs::path model_dir = fs::path(local_path_) / dir_name;
    fs::path partial_path = model_dir / "model.gguf.partial";
    fs::path final_path = model_dir / "model.gguf";

    std::error_code ec;
    fs::remove(partial_path, ec);  // cleanup stale partials (best-effort)

    ModelDownloader downloader(
        router_url_,
        local_path_,
        std::chrono::milliseconds(download_timeout_ms_),
        2,
        std::chrono::milliseconds(200),
        router_api_key_);

    const auto blob_path = std::string("/v0/models/blob/") + urlEncodePathSegment(model_name);
    const auto rel_partial = (fs::path(dir_name) / "model.gguf.partial").string();
    const auto downloaded = downloader.downloadBlob(blob_path, rel_partial, nullptr);
    if (downloaded.empty()) {
        fs::remove(partial_path, ec);
        return "";
    }

    if (!hasGgufMagic(downloaded)) {
        spdlog::warn("Router download returned non-GGUF for model {}", model_name);
        fs::remove(downloaded, ec);
        return "";
    }

    fs::rename(downloaded, final_path, ec);
    if (ec) {
        fs::remove(final_path, ec);
        ec.clear();
        fs::rename(downloaded, final_path, ec);
    }
    if (ec) {
        spdlog::error("Failed to finalize downloaded model {}: {}", model_name, ec.message());
        fs::remove(downloaded, ec);
        return "";
    }

    return final_path.string();
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
