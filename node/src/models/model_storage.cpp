// SPEC-dcaeaec4: ModelStorage implementation
// Simple model file management without LLM runtime dependency
#include "models/model_storage.h"

#include <filesystem>
#include <fstream>
#include <algorithm>
#include <cctype>
#include <spdlog/spdlog.h>

namespace fs = std::filesystem;
using json = nlohmann::json;

namespace llm_node {

namespace {
std::string sanitizeModelId(const std::string& input) {
    if (input.empty()) return "_latest";

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
        // Disallow path separators and other special characters by replacing them.
        out.push_back('_');
    }

    if (out.empty() || out == "." || out == "..") return "_latest";
    return out;
}
}  // namespace

ModelStorage::ModelStorage(std::string models_dir) : models_dir_(std::move(models_dir)) {}

std::string ModelStorage::modelNameToDir(const std::string& model_name) {
    return sanitizeModelId(model_name);
}

std::string ModelStorage::dirNameToModel(const std::string& dir_name) {
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

    for (const auto& entry : fs::directory_iterator(models_dir_)) {
        if (!entry.is_directory()) continue;

        const auto dir_name = entry.path().filename().string();
        const auto gguf_path = entry.path() / "model.gguf";

        std::error_code ec;
        auto st = fs::symlink_status(gguf_path, ec);
        if (st.type() != fs::file_type::regular && st.type() != fs::file_type::symlink) {
            spdlog::debug("ModelStorage::listAvailable: skipping {} (no model.gguf)", dir_name);
            continue;
        }

        ModelInfo info;
        info.name = dirNameToModel(dir_name);
        info.gguf_path = gguf_path.string();
        info.valid = true;

        out.push_back(std::move(info));
    }

    spdlog::debug("ModelStorage::listAvailable: found {} models", out.size());
    return out;
}

std::optional<nlohmann::json> ModelStorage::loadMetadata(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto metadata_path = fs::path(models_dir_) / dir_name / "metadata.json";

    if (!fs::exists(metadata_path)) {
        return std::nullopt;
    }

    try {
        std::ifstream ifs(metadata_path);
        json j = json::parse(ifs);
        return j;
    } catch (const std::exception& e) {
        spdlog::warn("ModelStorage::loadMetadata: failed to parse {}: {}", metadata_path.string(), e.what());
        return std::nullopt;
    }
}

bool ModelStorage::validateModel(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto gguf_path = fs::path(models_dir_) / dir_name / "model.gguf";

    std::error_code ec;
    auto st = fs::symlink_status(gguf_path, ec);
    return st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
}

}  // namespace llm_node
