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

    // SPEC-dcaeaec4 FR-2: 階層形式をサポートするため再帰的に走査
    for (const auto& entry : fs::recursive_directory_iterator(models_dir_)) {
        if (entry.is_directory()) continue;

        // model.gguf ファイルを検索
        if (entry.path().filename() != "model.gguf") continue;

        std::error_code ec;
        auto st = fs::symlink_status(entry.path(), ec);
        if (st.type() != fs::file_type::regular && st.type() != fs::file_type::symlink) {
            continue;
        }

        // 親ディレクトリからモデル名を計算
        // models_dir/openai/gpt-oss-20b/model.gguf → openai/gpt-oss-20b
        const auto parent_dir = entry.path().parent_path();
        const auto relative = fs::relative(parent_dir, models_dir_, ec);
        if (ec || relative.empty()) {
            spdlog::debug("ModelStorage::listAvailable: skipping {} (failed to compute relative path)", entry.path().string());
            continue;
        }

        const std::string model_name = relative.string();
        spdlog::debug("ModelStorage::listAvailable: found model {} at {}", model_name, entry.path().string());

        ModelInfo info;
        info.name = dirNameToModel(model_name);
        info.gguf_path = entry.path().string();
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
