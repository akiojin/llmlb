#include "core/kv_cache_utils.h"

#include <cstdlib>
#include <filesystem>

#include "utils/sha256.h"

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

std::string get_env_value(const char* key) {
    if (const char* value = std::getenv(key)) {
        return value;
    }
    return "";
}
}  // namespace

std::string get_default_kv_cache_dir() {
    std::string override = get_env_value("XLLM_KV_CACHE_DIR");
    if (!override.empty()) {
        return override;
    }
    return (fs::path(get_home_dir()) / ".xllm" / "cache").string();
}

std::string build_kv_cache_key(const std::string& model_id, const std::string& prompt) {
    return sha256_text(model_id + "\n" + prompt);
}

std::filesystem::path build_kv_cache_path(const std::string& model_id,
                                          const std::string& prompt,
                                          const std::string& base_dir) {
    const std::string dir = base_dir.empty() ? get_default_kv_cache_dir() : base_dir;
    const std::string key = build_kv_cache_key(model_id, prompt);
    if (key.empty()) {
        return {};
    }
    return fs::path(dir) / (key + ".session");
}

bool ensure_kv_cache_dir(const std::string& dir, std::string& error) {
    if (dir.empty()) {
        error = "KV cache directory is empty";
        return false;
    }
    std::error_code ec;
    if (fs::exists(dir, ec)) {
        return true;
    }
    if (!fs::create_directories(dir, ec)) {
        error = "Failed to create KV cache directory: " + dir;
        return false;
    }
    return true;
}

}  // namespace xllm
