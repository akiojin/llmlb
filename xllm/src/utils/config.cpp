#include "utils/config.h"
#include <cstdlib>
#include <filesystem>
#include <optional>
#include <algorithm>
#include <cctype>
#include <fstream>
#include <nlohmann/json.hpp>
#include <sstream>
#include <spdlog/spdlog.h>
#ifdef _WIN32
#include <windows.h>
#endif
#include "utils/file_lock.h"
#include "utils/allowlist.h"

namespace xllm {

namespace {

std::optional<std::string> getEnvValue(const char* name) {
    if (!name || !*name) {
        return std::nullopt;
    }
#ifdef _WIN32
    if (const char* v = std::getenv(name)) {
        return std::string(v);
    }
    DWORD size = GetEnvironmentVariableA(name, nullptr, 0);
    if (size == 0) {
        if (GetLastError() == ERROR_ENVVAR_NOT_FOUND) {
            return std::nullopt;
        }
        return std::string();
    }
    std::string value(size, '\0');
    DWORD copied = GetEnvironmentVariableA(name, value.data(), size);
    if (copied == 0 && GetLastError() == ERROR_ENVVAR_NOT_FOUND) {
        return std::nullopt;
    }
    value.resize(copied);
    return value;
#else
    if (const char* v = std::getenv(name)) {
        return std::string(v);
    }
    return std::nullopt;
#endif
}

/// Get environment variable with fallback to deprecated name
/// Logs a warning if the deprecated name is used
std::optional<std::string> getEnvWithFallback(const char* new_name, const char* old_name) {
    if (auto v = getEnvValue(new_name)) {
        return v;
    }
    if (auto v = getEnvValue(old_name)) {
        spdlog::warn("Environment variable '{}' is deprecated, use '{}' instead", old_name, new_name);
        return v;
    }
    return std::nullopt;
}

}  // namespace

DownloadConfig loadDownloadConfig() {
    auto info = loadDownloadConfigWithLog();
    return info.first;
}

std::pair<DownloadConfig, std::string> loadDownloadConfigWithLog() {
    DownloadConfig cfg;
    std::ostringstream log;
    bool used_file = false;
    bool used_env = false;

    // Optional JSON config file: path from LLM_DL_CONFIG or ~/.llm-router/config.json
    auto load_from_file = [&](const std::filesystem::path& path) {
        if (!std::filesystem::exists(path)) return false;
        try {
#ifndef _WIN32
            FileLock lock(path);
#endif
            std::ifstream ifs(path);
            if (!ifs.is_open()) return false;

            nlohmann::json j;
            ifs >> j;
            if (j.contains("max_retries")) cfg.max_retries = j.value("max_retries", cfg.max_retries);
            if (j.contains("backoff_ms")) cfg.backoff = std::chrono::milliseconds(j.value("backoff_ms", cfg.backoff.count()));
            if (j.contains("concurrency")) cfg.max_concurrency = j.value("concurrency", cfg.max_concurrency);
            if (j.contains("max_bps")) cfg.max_bytes_per_sec = j.value("max_bps", cfg.max_bytes_per_sec);
            if (j.contains("chunk")) cfg.chunk_size = j.value("chunk", cfg.chunk_size);
            log << "file=" << path << " ";
            return true;
        } catch (...) {
            return false;
        }
    };

    if (auto env = getEnvValue("LLM_DL_CONFIG")) {
        if (load_from_file(*env)) {
            used_file = true;
        }
    } else {
        try {
            std::filesystem::path home = getEnvValue("HOME").value_or("");
            auto path = home / std::filesystem::path(".llm-router/config.json");
            if (load_from_file(path)) {
                used_file = true;
            }
        } catch (...) {}
    }

    if (auto env = getEnvValue("LLM_DL_MAX_RETRIES")) {
        try {
            int v = std::stoi(*env);
            if (v >= 0) cfg.max_retries = v;
            log << "env:MAX_RETRIES=" << v << " ";
            used_env = true;
        } catch (...) {}
    }

    if (auto env = getEnvValue("LLM_DL_BACKOFF_MS")) {
        try {
            long long ms = std::stoll(*env);
            if (ms >= 0) cfg.backoff = std::chrono::milliseconds(ms);
            log << "env:BACKOFF_MS=" << ms << " ";
            used_env = true;
        } catch (...) {}
    }

    if (auto env = getEnvValue("LLM_DL_CONCURRENCY")) {
        try {
            long long v = std::stoll(*env);
            if (v > 0 && v < 64) cfg.max_concurrency = static_cast<size_t>(v);
            log << "env:CONCURRENCY=" << v << " ";
            used_env = true;
        } catch (...) {}
    }

    if (auto env = getEnvValue("LLM_DL_MAX_BPS")) {
        try {
            long long v = std::stoll(*env);
            if (v > 0) cfg.max_bytes_per_sec = static_cast<size_t>(v);
            log << "env:MAX_BPS=" << v << " ";
            used_env = true;
        } catch (...) {}
    }

    if (auto env = getEnvValue("LLM_DL_CHUNK")) {
        try {
            long long v = std::stoll(*env);
            if (v > 0 && v <= 1 << 20) cfg.chunk_size = static_cast<size_t>(v);
            log << "env:CHUNK=" << v << " ";
            used_env = true;
        } catch (...) {}
    }

    if (log.tellp() > 0) log << "|";
    log << "sources=";
    if (used_env) log << "env";
    if (used_file) {
        if (used_env) log << ",";
        log << "file";
    }
    if (!used_env && !used_file) log << "default";

    return {cfg, log.str()};
}

namespace {

std::filesystem::path defaultConfigPath() {
    try {
        std::filesystem::path home = getEnvValue("HOME").value_or("");
        if (!home.empty()) return home / ".llm-router/config.json";
    } catch (...) {
    }
    return std::filesystem::path();
}

bool readJsonWithLock(const std::filesystem::path& path, nlohmann::json& out) {
    if (!std::filesystem::exists(path)) return false;
#ifndef _WIN32
    FileLock lock(path);
#endif
    try {
        std::ifstream ifs(path);
        if (!ifs.is_open()) return false;
        ifs >> out;
        return true;
    } catch (...) {
        return false;
    }
}

}  // namespace

std::pair<NodeConfig, std::string> loadNodeConfigWithLog() {
    NodeConfig cfg;
    cfg.bind_address = "0.0.0.0";
    cfg.require_gpu = true;
    std::ostringstream log;
    bool used_env = false;
    bool used_file = false;

    // defaults: ~/.llm-router/models/
    cfg.models_dir = defaultConfigPath().empty()
                         ? ".llm-router/models"
                         : (defaultConfigPath().parent_path() / "models").string();
    cfg.origin_allowlist = splitAllowlistCsv("huggingface.co/*,cdn-lfs.huggingface.co/*");

    auto apply_json = [&](const nlohmann::json& j) {
        if (j.contains("models_dir") && j["models_dir"].is_string()) {
            cfg.models_dir = j["models_dir"].get<std::string>();
        }
        if (j.contains("node_port") && j["node_port"].is_number()) {
            cfg.node_port = j["node_port"].get<int>();
        }
        if (j.contains("bind_address") && j["bind_address"].is_string()) {
            cfg.bind_address = j["bind_address"].get<std::string>();
        }
        if (j.contains("origin_allowlist")) {
            const auto& v = j["origin_allowlist"];
            std::vector<std::string> list;
            if (v.is_array()) {
                for (const auto& item : v) {
                    if (item.is_string()) {
                        list.push_back(item.get<std::string>());
                    }
                }
            } else if (v.is_string()) {
                list = splitAllowlistCsv(v.get<std::string>());
            }
            if (!list.empty()) cfg.origin_allowlist = std::move(list);
        }
    };

    // file
    std::filesystem::path cfg_path;
    if (auto env = getEnvValue("XLLM_CONFIG")) {
        cfg_path = *env;
    } else {
        cfg_path = defaultConfigPath();
    }

    if (!cfg_path.empty()) {
        nlohmann::json j;
        if (readJsonWithLock(cfg_path, j)) {
            apply_json(j);
            log << "file=" << cfg_path << " ";
            used_file = true;
        }
    }

    // env overrides with fallback to deprecated names
    // New names: XLLM_*
    // Deprecated: LLM_* without NODE prefix

    if (auto v = getEnvWithFallback("XLLM_MODELS_DIR", "LLM_MODELS_DIR")) {
        cfg.models_dir = *v;
        log << "env:MODELS_DIR=" << *v << " ";
        used_env = true;
    }
    if (auto v = getEnvWithFallback("XLLM_PORT", "XLLM_PORT")) {
        // XLLM_PORT is already the correct name
        try {
            cfg.node_port = std::stoi(*v);
            log << "env:NODE_PORT=" << cfg.node_port << " ";
            used_env = true;
        } catch (...) {}
    }
    if (auto v = getEnvWithFallback("XLLM_BIND_ADDRESS", "LLM_BIND_ADDRESS")) {
        cfg.bind_address = *v;
        log << "env:BIND_ADDRESS=" << *v << " ";
        used_env = true;
    }
    if (auto v = getEnvWithFallback("XLLM_ORIGIN_ALLOWLIST", "LLM_ORIGIN_ALLOWLIST")) {
        auto list = splitAllowlistCsv(*v);
        if (!list.empty()) {
            cfg.origin_allowlist = std::move(list);
            log << "env:ORIGIN_ALLOWLIST=" << *v << " ";
            used_env = true;
        }
    }

    if (auto v = getEnvValue("LLM_DEFAULT_EMBEDDING_MODEL")) {
        cfg.default_embedding_model = *v;
        log << "env:DEFAULT_EMBEDDING_MODEL=" << *v << " ";
        used_env = true;
    }

    if (log.tellp() > 0) log << "|";
    log << "sources=";
    if (used_env) log << "env";
    if (used_file) {
        if (used_env) log << ",";
        log << "file";
    }
    if (!used_env && !used_file) log << "default";

    return {cfg, log.str()};
}

NodeConfig loadNodeConfig() {
    auto info = loadNodeConfigWithLog();
    return info.first;
}

}  // namespace xllm
