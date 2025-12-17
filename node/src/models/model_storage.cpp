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
    if (model_name.empty()) {
        return "_latest";
    }

    std::string result = model_name;

    // Replace all colons with underscores
    std::replace(result.begin(), result.end(), ':', '_');

    // Router compatibility:
    // - Filename-based ids (e.g. "gpt-oss-20b") are already versioned and should not get "_latest".
    // - Plain ids without ':' and without '-' get "_latest" (e.g. "llama3" -> "llama3_latest").
    // - Hugging Face ids (e.g. "org/model") are nested dirs and must not get "_latest".
    if (model_name.find('/') == std::string::npos && model_name.find(':') == std::string::npos &&
        model_name.find('-') == std::string::npos) {
        result += "_latest";
    }

    return result;
}

std::string ModelStorage::dirNameToModel(const std::string& dir_name) {
    // Lossy reverse conversion:
    // - Strip "_latest" suffix used for non-versioned ids.
    // - Otherwise keep directory name as model id (router uses filename-based ids).
    constexpr const char* kLatestSuffix = "_latest";
    if (dir_name.size() > std::char_traits<char>::length(kLatestSuffix) &&
        dir_name.rfind(kLatestSuffix) == dir_name.size() - std::char_traits<char>::length(kLatestSuffix)) {
        return dir_name.substr(0, dir_name.size() - std::char_traits<char>::length(kLatestSuffix));
    }
    return dir_name;
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

std::string ModelStorage::resolveOnnx(const std::string& model_name) const {
    const std::string dir_name = modelNameToDir(model_name);
    const auto onnx_path = fs::path(models_dir_) / dir_name / "model.onnx";

    std::error_code ec;
    auto st = fs::symlink_status(onnx_path, ec);
    const bool ok = st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
    spdlog::debug("ModelStorage::resolveOnnx: model={}, dir={}, path={}, exists={}",
        model_name, dir_name, onnx_path.string(), ok);

    if (ok) return onnx_path.string();

    return "";
}

std::vector<ModelInfo> ModelStorage::listAvailable() const {
    std::vector<ModelInfo> out;

    if (!fs::exists(models_dir_)) {
        spdlog::debug("ModelStorage::listAvailable: models_dir does not exist: {}", models_dir_);
        return out;
    }

    std::error_code ec;
    fs::recursive_directory_iterator it(models_dir_, ec);
    fs::recursive_directory_iterator end;
    for (; it != end && !ec; it.increment(ec)) {
        const auto& entry = *it;
        if (!entry.is_directory(ec) || ec) continue;

        const auto dir_path = entry.path();
        const auto onnx_path = dir_path / "model.onnx";
        const auto gguf_path = dir_path / "model.gguf";

        auto onnx_st = fs::symlink_status(onnx_path, ec);
        const bool has_onnx =
            onnx_st.type() == fs::file_type::regular || onnx_st.type() == fs::file_type::symlink;
        auto gguf_st = fs::symlink_status(gguf_path, ec);
        const bool has_gguf =
            gguf_st.type() == fs::file_type::regular || gguf_st.type() == fs::file_type::symlink;

        if (!has_onnx && !has_gguf) {
            continue;
        }

        // This directory is a model root; don't recurse further.
        it.disable_recursion_pending();

        // Derive model id from relative path:
        // - depth==1: legacy dir name (apply lossy reverse conversion)
        // - depth>=2: keep path segments joined by '/' (Hugging Face org/model, etc.)
        fs::path rel = fs::relative(dir_path, fs::path(models_dir_), ec);
        if (ec) {
            rel = dir_path.filename();
            ec.clear();
        }
        std::vector<std::string> parts;
        for (const auto& p : rel) {
            parts.push_back(p.string());
        }

        std::string model_id;
        if (parts.size() <= 1) {
            model_id = dirNameToModel(rel.string());
        } else {
            model_id = parts[0];
            for (size_t i = 1; i < parts.size(); ++i) {
                model_id += "/";
                model_id += parts[i];
            }
        }

        ModelInfo info;
        info.name = std::move(model_id);
        if (has_onnx) info.onnx_path = onnx_path.string();
        if (has_gguf) info.gguf_path = gguf_path.string();
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
    const auto onnx_path = fs::path(models_dir_) / dir_name / "model.onnx";
    const auto gguf_path = fs::path(models_dir_) / dir_name / "model.gguf";

    std::error_code ec;
    auto st_onnx = fs::symlink_status(onnx_path, ec);
    if (st_onnx.type() == fs::file_type::regular || st_onnx.type() == fs::file_type::symlink) {
        return true;
    }
    auto st = fs::symlink_status(gguf_path, ec);
    return st.type() == fs::file_type::regular || st.type() == fs::file_type::symlink;
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
