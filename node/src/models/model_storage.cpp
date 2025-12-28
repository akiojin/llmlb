// SPEC-dcaeaec4: ModelStorage implementation
// Simple model file management without LLM runtime dependency
#include "models/model_storage.h"

#include <filesystem>
#include <fstream>
#include <algorithm>
#include <cctype>
#include <unordered_map>
#include <unordered_set>
#include <spdlog/spdlog.h>

namespace fs = std::filesystem;
using json = nlohmann::json;

namespace llm_node {

namespace {
bool is_regular_or_symlink_file(const fs::path& path) {
    std::error_code ec;
    auto st = fs::symlink_status(path, ec);
    if (ec) return false;
    return st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
}

bool is_valid_file(const fs::path& path) {
    std::error_code ec;
    if (!is_regular_or_symlink_file(path)) return false;
    auto size = fs::file_size(path, ec);
    return !ec && size > 0;
}

bool has_required_safetensors_metadata(const fs::path& model_dir) {
    return is_valid_file(model_dir / "config.json") && is_valid_file(model_dir / "tokenizer.json");
}

bool validate_safetensors_index_shards(const fs::path& model_dir, const fs::path& index_path) {
    if (!is_valid_file(index_path)) return false;
    try {
        std::ifstream ifs(index_path);
        nlohmann::json j;
        ifs >> j;

        if (!j.contains("weight_map") || !j["weight_map"].is_object()) {
            return false;
        }

        const auto& weight_map = j["weight_map"];
        std::unordered_set<std::string> shard_files;
        for (auto it = weight_map.begin(); it != weight_map.end(); ++it) {
            if (!it.value().is_string()) continue;
            shard_files.insert(it.value().get<std::string>());
        }

        // Empty weight_map is allowed (e.g., placeholder index for tests).
        for (const auto& shard : shard_files) {
            const auto shard_path = model_dir / shard;
            if (!is_valid_file(shard_path)) {
                spdlog::warn("ModelStorage: missing safetensors shard: {}", shard_path.string());
                return false;
            }
        }
        return true;
    } catch (...) {
        return false;
    }
}

std::optional<std::string> detect_runtime_from_config(const fs::path& model_dir) {
    const auto cfg_path = model_dir / "config.json";
    if (!fs::exists(cfg_path)) return std::nullopt;
    try {
        std::ifstream ifs(cfg_path);
        nlohmann::json j;
        ifs >> j;

        if (j.contains("architectures") && j["architectures"].is_array()) {
            for (const auto& a : j["architectures"]) {
                if (!a.is_string()) continue;
                const auto s = a.get<std::string>();
                if (s.find("GptOss") != std::string::npos || s.find("GPTOSS") != std::string::npos) {
                    return std::string("gptoss_cpp");
                }
                if (s.find("Nemotron") != std::string::npos) {
                    return std::string("nemotron_cpp");
                }
            }
        }

        if (j.contains("model_type") && j["model_type"].is_string()) {
            auto mt = j["model_type"].get<std::string>();
            std::transform(mt.begin(), mt.end(), mt.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
            if (mt.find("gpt_oss") != std::string::npos || mt.find("gptoss") != std::string::npos) {
                return std::string("gptoss_cpp");
            }
            if (mt.find("nemotron") != std::string::npos) {
                return std::string("nemotron_cpp");
            }
        }
    } catch (...) {
        // ignore parse errors
    }
    return std::nullopt;
}

std::optional<fs::path> resolve_safetensors_primary_in_dir(const fs::path& model_dir) {
    if (!has_required_safetensors_metadata(model_dir)) return std::nullopt;

    std::vector<fs::path> index_files;
    std::vector<fs::path> safetensors_files;

    std::error_code ec;
    for (const auto& entry : fs::directory_iterator(model_dir, ec)) {
        if (ec) break;
        if (!entry.is_regular_file()) continue;

        const auto filename = entry.path().filename().string();
        const auto lower = [&]() {
            std::string s = filename;
            std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
            return s;
        }();

        const std::string kIndexSuffix = ".safetensors.index.json";
        const std::string kSafetensorsSuffix = ".safetensors";

        const bool is_index = lower.size() >= kIndexSuffix.size() &&
                              lower.rfind(kIndexSuffix) == lower.size() - kIndexSuffix.size();
        const bool is_safetensors = lower.size() >= kSafetensorsSuffix.size() &&
                                    lower.rfind(kSafetensorsSuffix) == lower.size() - kSafetensorsSuffix.size();

        if (is_index) {
            if (is_valid_file(entry.path())) {
                index_files.push_back(entry.path());
            }
            continue;
        }

        if (is_safetensors) {
            // シャードも含むが、indexがある場合は index を優先する
            if (is_valid_file(entry.path())) {
                safetensors_files.push_back(entry.path());
            }
            continue;
        }
    }

    if (index_files.size() == 1) {
        if (!validate_safetensors_index_shards(model_dir, index_files[0])) {
            return std::nullopt;
        }
        return index_files[0];
    }
    if (!index_files.empty()) {
        return std::nullopt;  // ambiguous
    }

    // index が無い場合は単一 safetensors のみ許可
    if (safetensors_files.size() == 1) {
        return safetensors_files[0];
    }
    return std::nullopt;
}

/// モデルIDをサニタイズ
/// SPEC-dcaeaec4 FR-2: 階層形式を許可
/// - `gpt-oss-20b` → `gpt-oss-20b`
/// - `openai/gpt-oss-20b` → `openai/gpt-oss-20b`（ネストディレクトリ）
///
/// `/` はディレクトリセパレータとして保持し、危険なパターンは除去。
std::string sanitizeModelId(const std::string& input) {
    if (input.empty()) return "_latest";

    // 危険なパターンを検出
    if (input.find("..") != std::string::npos) return "_latest";
    if (input.find('\0') != std::string::npos) return "_latest";

    std::string out;
    out.reserve(input.size());
    for (unsigned char c : input) {
        if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-' || c == '_' || c == '.') {
            out.push_back(static_cast<char>(c));
            continue;
        }
        if (c >= 'A' && c <= 'Z') {
            out.push_back(static_cast<char>(std::tolower(c)));
            continue;
        }
        // `/` はディレクトリセパレータとして許可
        if (c == '/') {
            out.push_back('/');
            continue;
        }
        // その他の特殊文字は `_` に置換
        out.push_back('_');
    }

    // 先頭・末尾のスラッシュを除去
    size_t start = 0;
    size_t end = out.size();
    while (start < end && out[start] == '/') ++start;
    while (end > start && out[end - 1] == '/') --end;
    out = out.substr(start, end - start);

    if (out.empty() || out == "." || out == "..") return "_latest";
    return out;
}
}  // namespace

ModelStorage::ModelStorage(std::string models_dir) : models_dir_(std::move(models_dir)) {}

std::string ModelStorage::modelNameToDir(const std::string& model_name) {
    return sanitizeModelId(model_name);
}

std::string ModelStorage::dirNameToModel(const std::string& dir_name) {
    // 一貫性のため、ディレクトリ名もサニタイズして小文字に正規化
    return sanitizeModelId(dir_name);
}

std::string ModelStorage::resolveGguf(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto gguf_path = fs::path(models_dir_) / dir_name / "model.gguf";

    std::error_code ec;
    auto st = fs::symlink_status(gguf_path, ec);
    const bool ok = st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
    spdlog::debug("ModelStorage::resolveGguf: model={}, dir={}, path={}, exists={}",
        model_name, dir_name, gguf_path.string(), ok);

    if (ok) return gguf_path.string();

    return "";
}

std::vector<ModelInfo> ModelStorage::listAvailable() const {
    std::vector<ModelInfo> out;

    if (!fs::exists(models_dir_)) {
        spdlog::debug("ModelStorage::listAvailable: models_dir does not exist: {}", models_dir_);
        return out;
    }

    // SPEC-dcaeaec4 FR-2: 階層形式をサポートするため再帰的に走査（ディレクトリ単位）
    std::error_code ec;
    for (const auto& entry : fs::recursive_directory_iterator(models_dir_, ec)) {
        if (ec) break;
        if (!entry.is_directory()) continue;

        const auto model_dir = entry.path();
        const auto relative = fs::relative(model_dir, models_dir_, ec);
        if (ec || relative.empty()) {
            ec.clear();
            continue;
        }

        // GGUF
        const auto gguf_path = model_dir / "model.gguf";
        if (is_valid_file(gguf_path)) {
            ModelInfo info;
            info.name = dirNameToModel(relative.string());
            info.format = "gguf";
            info.primary_path = gguf_path.string();
            info.valid = true;
            out.push_back(std::move(info));
            continue;
        }

        // safetensors
        if (auto primary = resolve_safetensors_primary_in_dir(model_dir)) {
            ModelInfo info;
            info.name = dirNameToModel(relative.string());
            info.format = "safetensors";
            info.primary_path = primary->string();
            info.valid = true;
            out.push_back(std::move(info));
            continue;
        }
    }

    spdlog::debug("ModelStorage::listAvailable: found {} models", out.size());
    return out;
}

std::vector<ModelDescriptor> ModelStorage::listAvailableDescriptors() const {
    std::vector<ModelDescriptor> out;
    for (const auto& info : listAvailable()) {
        ModelDescriptor desc;
        desc.name = info.name;
        desc.format = info.format;
        desc.primary_path = info.primary_path;
        desc.model_dir = fs::path(info.primary_path).parent_path().string();

        if (info.format == "gguf") {
            desc.runtime = "llama_cpp";
            out.push_back(std::move(desc));
            continue;
        }

        if (info.format == "safetensors") {
            auto rt = detect_runtime_from_config(fs::path(desc.model_dir));
            if (!rt) continue;
            desc.runtime = *rt;
            out.push_back(std::move(desc));
            continue;
        }
    }
    return out;
}

std::optional<ModelDescriptor> ModelStorage::resolveDescriptor(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto model_dir = fs::path(models_dir_) / dir_name;

    const auto gguf_path = model_dir / "model.gguf";
    if (is_valid_file(gguf_path)) {
        ModelDescriptor desc;
        desc.name = model_name;
        desc.runtime = "llama_cpp";
        desc.format = "gguf";
        desc.primary_path = gguf_path.string();
        desc.model_dir = model_dir.string();
        return desc;
    }

    if (auto primary = resolve_safetensors_primary_in_dir(model_dir)) {
        auto rt = detect_runtime_from_config(model_dir);
        if (!rt) return std::nullopt;
        ModelDescriptor desc;
        desc.name = model_name;
        desc.runtime = *rt;
        desc.format = "safetensors";
        desc.primary_path = primary->string();
        desc.model_dir = model_dir.string();
        return desc;
    }

    return std::nullopt;
}

bool ModelStorage::validateModel(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto model_dir = fs::path(models_dir_) / dir_name;
    if (is_valid_file(model_dir / "model.gguf")) return true;
    return resolve_safetensors_primary_in_dir(model_dir).has_value();
}

bool ModelStorage::deleteModel(const std::string& model_name) {
    const std::string dir_name = modelNameToDir(model_name);
    const auto model_dir = fs::path(models_dir_) / dir_name;

    if (!fs::exists(model_dir)) {
        spdlog::debug("ModelStorage::deleteModel: model directory does not exist: {}", model_dir.string());
        return true;  // Already deleted
    }

    std::error_code ec;
    fs::remove_all(model_dir, ec);
    if (ec) {
        spdlog::error("ModelStorage::deleteModel: failed to delete {}: {}", model_dir.string(), ec.message());
        return false;
    }

    spdlog::info("ModelStorage::deleteModel: deleted model directory: {}", model_dir.string());
    return true;
}

}  // namespace llm_node
