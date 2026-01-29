#include "core/lora_utils.h"

#include <cstdlib>
#include <filesystem>

namespace fs = std::filesystem;

namespace xllm {

namespace {
std::string get_home_dir() {
    if (const char* home = std::getenv("HOME")) {
        return home;
    }
    if (const char* userprofile = std::getenv("USERPROFILE")) {
        return userprofile;
    }
    return ".";
}

bool resolve_single_lora(const LoraRequest& request,
                         const std::string& base_dir,
                         LoraRequest& resolved,
                         std::string& error) {
    resolved = request;

    fs::path path;
    if (!resolved.path.empty()) {
        path = fs::path(resolved.path);
    } else if (!resolved.name.empty()) {
        path = fs::path(resolved.name);
    } else {
        error = "lora name or path is required";
        return false;
    }

    if (!path.is_absolute()) {
        fs::path base = base_dir.empty() ? fs::path(".") : fs::path(base_dir);
        path = base / path;
    }

    std::error_code ec;
    fs::path abs = fs::absolute(path, ec);
    if (!ec) {
        path = abs;
    }

    if (!fs::exists(path)) {
        error = "LoRA file not found: " + path.string();
        return false;
    }

    resolved.path = path.string();
    if (resolved.name.empty()) {
        resolved.name = path.filename().string();
    }

    return true;
}
}  // namespace

std::string get_default_lora_dir() {
    return (fs::path(get_home_dir()) / ".xllm" / "loras").string();
}

std::vector<LoraRequest> resolve_lora_requests(
    const std::vector<LoraRequest>& requests,
    const std::string& base_dir,
    std::string& error) {
    std::vector<LoraRequest> resolved;
    resolved.reserve(requests.size());
    for (const auto& request : requests) {
        LoraRequest entry;
        if (!resolve_single_lora(request, base_dir, entry, error)) {
            return {};
        }
        resolved.push_back(std::move(entry));
    }
    return resolved;
}

}  // namespace xllm
