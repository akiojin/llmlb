// SPEC-48678000: ModelResolver implementation
// Resolves model paths with fallback: local -> shared -> router API download
#include "models/model_resolver.h"

#include <chrono>
#include <filesystem>
#include <fstream>
#include <memory>
#include <thread>
#include <regex>

#include <httplib.h>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#include "models/model_downloader.h"
#include "models/model_storage.h"
#include "utils/allowlist.h"

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

struct ParsedUrl {
    std::string scheme;
    std::string host;
    int port{0};
    std::string path;
};

ParsedUrl parseUrl(const std::string& url) {
    static const std::regex re(R"(^([a-zA-Z][a-zA-Z0-9+.-]*)://([^/:]+)(?::(\d+))?(.*)$)");
    std::smatch match;
    ParsedUrl parsed;
    if (std::regex_match(url, match, re)) {
        parsed.scheme = match[1].str();
        parsed.host = match[2].str();
        parsed.port = match[3].matched ? std::stoi(match[3].str()) : (parsed.scheme == "https" ? 443 : 80);
        parsed.path = match[4].str().empty() ? "/" : match[4].str();
    }
    return parsed;
}

std::unique_ptr<httplib::Client> makeClient(const ParsedUrl& url, std::chrono::milliseconds timeout) {
    if (url.scheme.empty() || url.host.empty()) {
        return nullptr;
    }

#ifndef CPPHTTPLIB_OPENSSL_SUPPORT
    if (url.scheme == "https") {
        return nullptr;  // HTTPS is not supported in this build
    }
#endif

    std::string scheme_host_port = url.scheme + "://" + url.host;
    if (url.port != 0) {
        scheme_host_port += ":" + std::to_string(url.port);
    }
    auto client = std::make_unique<httplib::Client>(scheme_host_port);
    if (client && client->is_valid()) {
        const int sec = static_cast<int>(timeout.count() / 1000);
        const int usec = static_cast<int>((timeout.count() % 1000) * 1000);
        client->set_connection_timeout(sec, usec);
        client->set_read_timeout(sec, usec);
        client->set_write_timeout(sec, usec);
        client->set_follow_location(true);
        return client;
    }
    return nullptr;
}

std::string joinPath(const std::string& base, const std::string& tail) {
    std::string out = base.empty() ? "/" : base;
    if (out.back() != '/') out.push_back('/');
    if (!tail.empty() && tail.front() == '/') {
        out += tail.substr(1);
    } else {
        out += tail;
    }
    return out;
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
        bool origin_attempted = false;
        std::string downloaded = downloadFromRouter(model_name, &origin_attempted);
        if (!downloaded.empty()) {
            result.success = true;
            result.path = downloaded;
            result.origin_attempted = origin_attempted;
            return result;
        }
        result.origin_attempted = origin_attempted;
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

void ModelResolver::setOriginAllowlist(std::vector<std::string> origin_allowlist) {
    origin_allowlist_ = std::move(origin_allowlist);
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

std::string ModelResolver::downloadFromRouter(const std::string& model_name, bool* origin_attempted) {
    if (router_url_.empty() || local_path_.empty()) return "";
    if (origin_attempted) *origin_attempted = false;

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
        if (origin_attempted) *origin_attempted = false;
        if (auto origin_url = fetchOriginUrl(model_name)) {
            if (isUrlAllowedByAllowlist(*origin_url, origin_allowlist_)) {
                if (origin_attempted) *origin_attempted = true;
                return downloadFromOrigin(model_name, *origin_url);
            }
            spdlog::warn("Origin URL blocked by allowlist for model {}", model_name);
        }
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

std::string ModelResolver::downloadFromOrigin(const std::string& model_name, const std::string& url) {
    if (url.empty() || local_path_.empty()) return "";

    const auto dir_name = ModelStorage::modelNameToDir(model_name);
    fs::path model_dir = fs::path(local_path_) / dir_name;
    fs::path partial_path = model_dir / "model.gguf.partial";
    fs::path final_path = model_dir / "model.gguf";

    std::error_code ec;
    fs::remove(partial_path, ec);

    ModelDownloader downloader(
        router_url_,
        local_path_,
        std::chrono::milliseconds(download_timeout_ms_),
        2,
        std::chrono::milliseconds(200),
        router_api_key_);

    const auto rel_partial = (fs::path(dir_name) / "model.gguf.partial").string();
    const auto downloaded = downloader.downloadBlob(url, rel_partial, nullptr);
    if (downloaded.empty()) {
        fs::remove(partial_path, ec);
        return "";
    }

    if (!hasGgufMagic(downloaded)) {
        spdlog::warn("Origin download returned non-GGUF for model {}", model_name);
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
        spdlog::error("Failed to finalize origin download for model {}: {}", model_name, ec.message());
        fs::remove(downloaded, ec);
        return "";
    }

    return final_path.string();
}

std::optional<std::string> ModelResolver::fetchOriginUrl(const std::string& model_name) {
    if (router_url_.empty()) return std::nullopt;

    auto parsed = parseUrl(router_url_);
    auto client = makeClient(parsed, std::chrono::milliseconds(2000));
    if (!client) return std::nullopt;

    const std::string path = joinPath(parsed.path, "/v0/models");
    httplib::Headers headers;
    if (!router_api_key_.empty()) {
        headers.emplace("Authorization", "Bearer " + router_api_key_);
    }

    auto res = client->Get(path.c_str(), headers);
    if (!res || res->status < 200 || res->status >= 300) {
        return std::nullopt;
    }

    nlohmann::json body = nlohmann::json::parse(res->body, nullptr, false);
    if (body.is_discarded()) return std::nullopt;

    const nlohmann::json* arr = nullptr;
    if (body.is_array()) {
        arr = &body;
    } else if (body.contains("data") && body["data"].is_array()) {
        arr = &body["data"];
    }

    if (!arr) return std::nullopt;

    for (const auto& m : *arr) {
        if (!m.is_object()) continue;
        std::string id;
        if (m.contains("name") && m["name"].is_string()) {
            id = m["name"].get<std::string>();
        } else if (m.contains("id") && m["id"].is_string()) {
            id = m["id"].get<std::string>();
        }
        if (id != model_name) continue;

        if (m.contains("download_url") && m["download_url"].is_string()) {
            auto url = m["download_url"].get<std::string>();
            if (!url.empty()) return url;
        }
        if (m.contains("repo") && m["repo"].is_string() && m.contains("filename") && m["filename"].is_string()) {
            const std::string repo = m["repo"].get<std::string>();
            const std::string filename = m["filename"].get<std::string>();
            if (!repo.empty() && !filename.empty()) {
                std::string base = std::getenv("HF_BASE_URL") ? std::getenv("HF_BASE_URL") : "https://huggingface.co";
                if (!base.empty() && base.back() == '/') base.pop_back();
                return base + "/" + repo + "/resolve/main/" + filename;
            }
        }
        break;
    }

    return std::nullopt;
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
