#include "core/vision_processor.h"

#include <algorithm>
#include <chrono>
#include <cctype>
#include <filesystem>
#include <regex>
#include <thread>

#include <httplib.h>
#include <spdlog/spdlog.h>

#include "models/model_storage.h"
#include "mtmd-helper.h"
#include "include/llama.h"

namespace fs = std::filesystem;

namespace llm_node {

namespace {
constexpr size_t kMaxImageBytes = 10 * 1024 * 1024;
constexpr std::chrono::milliseconds kImageTimeout{30000};

struct ParsedUrl {
    std::string scheme;
    std::string host;
    int port{0};
    std::string path;
};

std::optional<ParsedUrl> parseUrl(const std::string& url) {
    static const std::regex re(R"(^([a-zA-Z][a-zA-Z0-9+.-]*)://([^/:]+)(?::(\d+))?(.*)$)");
    std::smatch match;
    if (!std::regex_match(url, match, re)) {
        return std::nullopt;
    }
    ParsedUrl parsed;
    parsed.scheme = match[1].str();
    parsed.host = match[2].str();
    parsed.port = match[3].matched ? std::stoi(match[3].str()) : (parsed.scheme == "https" ? 443 : 80);
    parsed.path = match[4].str().empty() ? "/" : match[4].str();
    if (!parsed.path.empty() && parsed.path[0] != '/') {
        parsed.path = "/" + parsed.path;
    }
    return parsed;
}

std::unique_ptr<httplib::Client> makeClient(const ParsedUrl& url) {
#ifndef CPPHTTPLIB_OPENSSL_SUPPORT
    if (url.scheme == "https") {
        return nullptr;
    }
#endif

    // Build scheme://host:port format for Client's universal interface
    std::string scheme_host_port = url.scheme + "://" + url.host;
    if (url.port != 0) {
        scheme_host_port += ":" + std::to_string(url.port);
    }

    auto client = std::make_unique<httplib::Client>(scheme_host_port);
    if (client && client->is_valid()) {
        const int sec = static_cast<int>(kImageTimeout.count() / 1000);
        const int usec = static_cast<int>((kImageTimeout.count() % 1000) * 1000);
        client->set_connection_timeout(sec, usec);
        client->set_read_timeout(sec, usec);
        client->set_write_timeout(sec, usec);
        client->set_follow_location(true);
        return client;
    }

    return nullptr;
}

std::string toLower(std::string input) {
    std::transform(input.begin(), input.end(), input.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return input;
}

}  // namespace

VisionProcessor::VisionProcessor(const ModelStorage& model_storage)
    : model_storage_(model_storage) {}

const char* VisionProcessor::marker() {
    return mtmd_default_marker();
}

mtmd_context* VisionProcessor::getOrCreateContext(const std::string& model_name,
                                                  const std::string& model_path,
                                                  const llama_model* model,
                                                  std::string& error) const {
    auto mmproj_path = resolveMmprojPath(model_name, model_path);
    if (!mmproj_path) {
        error = "mmproj file not found for vision model";
        return nullptr;
    }

    const std::string key = model_path + "::" + *mmproj_path;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = contexts_.find(key);
        if (it != contexts_.end()) {
            return it->second.get();
        }
    }

    mtmd_context_params params = mtmd_context_params_default();
    params.use_gpu = true;
    params.print_timings = false;
    params.n_threads = std::max(1u, std::thread::hardware_concurrency());
    params.media_marker = mtmd_default_marker();

    mtmd_context* ctx = mtmd_init_from_file(mmproj_path->c_str(), model, params);
    if (!ctx) {
        error = "failed to initialize multimodal projector";
        return nullptr;
    }
    if (!mtmd_support_vision(ctx)) {
        mtmd_free(ctx);
        error = "mmproj does not support vision input";
        return nullptr;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        contexts_.emplace(key, mtmd::context_ptr(ctx));
    }

    spdlog::info("VisionProcessor: loaded mmproj {}", *mmproj_path);
    return ctx;
}

bool VisionProcessor::prepareBitmaps(mtmd_context* ctx,
                                     const std::vector<std::string>& image_urls,
                                     mtmd::bitmaps& out,
                                     std::string& error) const {
    out.entries.clear();
    out.entries.reserve(image_urls.size());

    for (const auto& url : image_urls) {
        std::vector<uint8_t> bytes;
        if (!loadImageData(url, bytes, error)) {
            return false;
        }
        if (bytes.size() > kMaxImageBytes) {
            error = "image exceeds maximum size";
            return false;
        }
        mtmd_bitmap* bmp = mtmd_helper_bitmap_init_from_buf(ctx, bytes.data(), bytes.size());
        if (!bmp) {
            error = "failed to decode image data";
            return false;
        }
        mtmd::bitmap bitmap(bmp);
        bitmap.set_id(url.c_str());
        out.entries.push_back(std::move(bitmap));
    }

    return true;
}

std::optional<std::string> VisionProcessor::resolveMmprojPath(const std::string& model_name,
                                                              const std::string& model_path) const {
    const fs::path model_dir = fs::path(model_path).parent_path();

    if (auto descriptor = model_storage_.resolveDescriptor(model_name); descriptor && descriptor->metadata) {
        const auto& metadata = *descriptor->metadata;
        static const char* kKeys[] = {"mmproj_path", "mmproj", "mmproj_file"};
        for (const auto* key : kKeys) {
            auto it = metadata.find(key);
            if (it == metadata.end() || !it->is_string()) {
                continue;
            }
            fs::path candidate = it->get<std::string>();
            if (candidate.empty()) {
                continue;
            }
            if (candidate.is_relative()) {
                candidate = model_dir / candidate;
            }
            std::error_code ec;
            if (fs::exists(candidate, ec)) {
                return candidate.string();
            }
        }
    }

    std::vector<fs::path> candidates;
    std::error_code ec;
    if (!fs::exists(model_dir, ec)) {
        return std::nullopt;
    }
    for (const auto& entry : fs::directory_iterator(model_dir, ec)) {
        if (ec) break;
        if (!entry.is_regular_file()) continue;
        if (entry.path().extension() != ".gguf") continue;
        std::string filename = toLower(entry.path().filename().string());
        if (filename.find("mmproj") == std::string::npos) continue;
        candidates.push_back(entry.path());
    }

    if (candidates.empty()) {
        return std::nullopt;
    }

    std::sort(candidates.begin(), candidates.end());
    return candidates.front().string();
}

bool VisionProcessor::loadImageData(const std::string& url,
                                    std::vector<uint8_t>& out,
                                    std::string& error) const {
    if (url.rfind("data:", 0) == 0) {
        return decodeDataUrl(url, out, error);
    }
    if (url.rfind("http://", 0) == 0 || url.rfind("https://", 0) == 0) {
        return fetchHttpUrl(url, out, error);
    }
    return decodeBase64(url, out, error);
}

bool VisionProcessor::decodeDataUrl(const std::string& url,
                                    std::vector<uint8_t>& out,
                                    std::string& error) {
    const std::string marker = "base64,";
    auto pos = url.find(marker);
    if (pos == std::string::npos) {
        error = "data URL is not base64 encoded";
        return false;
    }
    std::string encoded = url.substr(pos + marker.size());
    if (encoded.empty()) {
        error = "data URL payload is empty";
        return false;
    }
    return decodeBase64(encoded, out, error);
}

bool VisionProcessor::decodeBase64(const std::string& encoded,
                                   std::vector<uint8_t>& out,
                                   std::string& error) {
    static const std::string kBase64Chars =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        "abcdefghijklmnopqrstuvwxyz"
        "0123456789+/";

    std::string clean;
    clean.reserve(encoded.size());
    for (unsigned char c : encoded) {
        if (std::isspace(c)) continue;
        if (c == '-') c = '+';
        if (c == '_') c = '/';
        clean.push_back(static_cast<char>(c));
    }

    if (clean.empty()) {
        error = "base64 payload is empty";
        return false;
    }

    out.clear();
    int val = 0;
    int valb = -8;
    for (unsigned char c : clean) {
        if (c == '=') break;
        auto pos = kBase64Chars.find(static_cast<char>(c));
        if (pos == std::string::npos) {
            error = "invalid base64 payload";
            return false;
        }
        val = (val << 6) + static_cast<int>(pos);
        valb += 6;
        if (valb >= 0) {
            out.push_back(static_cast<uint8_t>((val >> valb) & 0xFF));
            valb -= 8;
        }
    }

    if (out.empty()) {
        error = "base64 decode produced empty payload";
        return false;
    }
    return true;
}

bool VisionProcessor::fetchHttpUrl(const std::string& url,
                                   std::vector<uint8_t>& out,
                                   std::string& error) {
    auto parsed = parseUrl(url);
    if (!parsed) {
        error = "invalid image URL";
        return false;
    }
    auto client = makeClient(*parsed);
    if (!client) {
        error = "HTTPS is not supported in this build";
        return false;
    }
    auto res = client->Get(parsed->path.c_str());
    if (!res) {
        error = "failed to fetch image URL";
        return false;
    }
    if (res->status < 200 || res->status >= 300) {
        error = "image URL returned HTTP " + std::to_string(res->status);
        return false;
    }

    out.assign(res->body.begin(), res->body.end());
    return !out.empty();
}

}  // namespace llm_node
