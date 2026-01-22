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

namespace xllm {

namespace {
std::optional<ParsedModelName> parse_model_name_with_quantization(const std::string& model_name) {
    if (model_name.empty()) return std::nullopt;
    auto pos = model_name.find(':');
    if (pos == std::string::npos) {
        return ParsedModelName{model_name, std::nullopt};
    }
    if (model_name.find(':', pos + 1) != std::string::npos) {
        return std::nullopt;
    }
    if (pos == 0 || pos == model_name.size() - 1) {
        return std::nullopt;
    }
    return ParsedModelName{
        model_name.substr(0, pos),
        model_name.substr(pos + 1),
    };
}

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

bool is_safetensors_index_file(const fs::path& path) {
    const std::string filename = path.filename().string();
    const std::string suffix = ".safetensors.index.json";
    if (filename.size() < suffix.size()) return false;
    std::string lower = filename;
    std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return lower.rfind(suffix) == lower.size() - suffix.size();
}

bool has_required_safetensors_metadata(const fs::path& model_dir) {
    return is_valid_file(model_dir / "config.json") && is_valid_file(model_dir / "tokenizer.json");
}

bool ends_with_case_insensitive(const std::string& value, const std::string& suffix) {
    if (value.size() < suffix.size()) return false;
    const size_t offset = value.size() - suffix.size();
    for (size_t i = 0; i < suffix.size(); ++i) {
        const auto lhs = static_cast<unsigned char>(value[offset + i]);
        const auto rhs = static_cast<unsigned char>(suffix[i]);
        if (std::tolower(lhs) != std::tolower(rhs)) return false;
    }
    return true;
}

bool is_gguf_filename(const std::string& filename) {
    return ends_with_case_insensitive(filename, ".gguf");
}

std::vector<fs::path> list_gguf_files(const fs::path& model_dir) {
    std::vector<fs::path> out;
    std::error_code ec;
    for (const auto& entry : fs::directory_iterator(model_dir, ec)) {
        if (ec) break;
        // シンボリックリンクもサポート（HuggingFaceキャッシュからのリンク対応）
        if (!is_regular_or_symlink_file(entry.path())) continue;
        const auto filename = entry.path().filename().string();
        if (!is_gguf_filename(filename)) continue;
        if (!is_valid_file(entry.path())) continue;
        out.push_back(entry.path());
    }
    std::sort(out.begin(), out.end());
    return out;
}

std::vector<std::string> build_gguf_prefixes(const std::string& model_name) {
    std::vector<std::string> prefixes;
    const std::string leaf = fs::path(model_name).filename().string();
    if (!leaf.empty()) {
        prefixes.push_back(leaf);
    }
    if (std::find(prefixes.begin(), prefixes.end(), "model") == prefixes.end()) {
        prefixes.push_back("model");
    }
    return prefixes;
}

std::optional<std::string> extract_quantization_token(
    const std::string& stem,
    const std::vector<std::string>& prefixes) {
    for (const auto& prefix : prefixes) {
        if (stem.size() <= prefix.size() + 1) continue;
        if (stem.compare(0, prefix.size(), prefix) != 0) continue;
        const char sep = stem[prefix.size()];
        if (sep != '.' && sep != '-') continue;
        const std::string token = stem.substr(prefix.size() + 1);
        if (!token.empty()) return token;
    }
    return std::nullopt;
}

std::optional<fs::path> resolve_gguf_with_quantization(
    const fs::path& model_dir,
    const std::string& model_name,
    const std::string& quantization) {
    if (quantization.empty()) return std::nullopt;
    auto files = list_gguf_files(model_dir);
    if (files.empty()) return std::nullopt;

    const auto prefixes = build_gguf_prefixes(model_name);
    std::optional<fs::path> match;
    for (const auto& file : files) {
        const auto stem = file.stem().string();
        const auto token = extract_quantization_token(stem, prefixes);
        if (!token || *token != quantization) continue;
        if (match.has_value()) {
            return std::nullopt;
        }
        match = file;
    }
    return match;
}

std::optional<fs::path> resolve_single_gguf_file(const fs::path& model_dir) {
    auto files = list_gguf_files(model_dir);
    if (files.size() == 1) return files.front();
    return std::nullopt;
}

std::optional<std::string> load_manifest_quantization(const fs::path& model_dir) {
    const auto manifest_path = model_dir / "manifest.json";
    if (!is_regular_or_symlink_file(manifest_path)) return std::nullopt;
    try {
        std::ifstream ifs(manifest_path);
        json j;
        ifs >> j;
        if (!j.contains("quantization")) return std::nullopt;
        if (!j["quantization"].is_string()) return std::nullopt;
        const auto value = j["quantization"].get<std::string>();
        if (value.empty()) return std::nullopt;
        return value;
    } catch (...) {
        return std::nullopt;
    }
}

void apply_manifest_quantization(ModelDescriptor& desc, const fs::path& model_dir) {
    auto quant = load_manifest_quantization(model_dir);
    if (!quant) return;
    if (!desc.metadata.has_value() || !desc.metadata->is_object()) {
        desc.metadata = nlohmann::json::object();
    }
    (*desc.metadata)["quantization"] = *quant;
}

void apply_quantization_request(ModelDescriptor& desc, const std::optional<std::string>& request) {
    std::optional<std::string> effective = request;
    if (!effective && desc.metadata && desc.metadata->contains("quantization")) {
        const auto& q = (*desc.metadata)["quantization"];
        if (q.is_string()) {
            effective = q.get<std::string>();
        }
    }
    if (!effective) return;
    if (!desc.metadata.has_value() || !desc.metadata->is_object()) {
        desc.metadata = nlohmann::json::object();
    }
    (*desc.metadata)["quantization_request"] = *effective;
}

std::optional<fs::path> resolve_gguf_for_quantization(
    const fs::path& model_dir,
    const std::string& model_name,
    const std::string& quantization,
    const std::optional<std::string>& manifest_quantization) {
    if (quantization.empty()) return std::nullopt;
    if (auto match = resolve_gguf_with_quantization(model_dir, model_name, quantization)) {
        return match;
    }
    if (manifest_quantization && *manifest_quantization == quantization) {
        const auto default_path = model_dir / "model.gguf";
        if (is_valid_file(default_path)) return default_path;
        if (auto single = resolve_single_gguf_file(model_dir)) return single;
    }
    return std::nullopt;
}

std::optional<fs::path> resolve_gguf_path(
    const fs::path& model_dir,
    const std::string& model_name,
    const std::string& quantization) {
    const auto manifest_quantization = load_manifest_quantization(model_dir);
    if (!quantization.empty()) {
        return resolve_gguf_for_quantization(model_dir, model_name, quantization, manifest_quantization);
    }
    if (manifest_quantization) {
        return resolve_gguf_for_quantization(
            model_dir,
            model_name,
            *manifest_quantization,
            manifest_quantization);
    }

    const auto default_path = model_dir / "model.gguf";
    if (is_valid_file(default_path)) return default_path;

    const std::string leaf = fs::path(model_name).filename().string();
    if (!leaf.empty()) {
        const auto leaf_path = model_dir / (leaf + ".gguf");
        if (is_valid_file(leaf_path)) return leaf_path;
    }

    return resolve_single_gguf_file(model_dir);
}

std::vector<std::string> load_manifest_formats(const fs::path& model_dir) {
    const auto manifest_path = model_dir / "manifest.json";
    if (!is_regular_or_symlink_file(manifest_path)) return {};
    try {
        std::ifstream ifs(manifest_path);
        json j;
        ifs >> j;
        auto normalize_format = [](std::string value) -> std::optional<std::string> {
            std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) {
                return static_cast<char>(std::tolower(c));
            });
            if (value == "gguf" || value == "safetensors") return value;
            return std::nullopt;
        };

        std::vector<std::string> out;
        auto push_unique = [&out](const std::string& value) {
            if (std::find(out.begin(), out.end(), value) == out.end()) {
                out.push_back(value);
            }
        };

        if (j.contains("formats") && j["formats"].is_array()) {
            for (const auto& entry : j["formats"]) {
                if (!entry.is_string()) continue;
                if (auto fmt = normalize_format(entry.get<std::string>())) {
                    push_unique(*fmt);
                }
            }
            return out;
        }

        if (j.contains("format") && j["format"].is_string()) {
            if (auto fmt = normalize_format(j["format"].get<std::string>())) {
                push_unique(*fmt);
            }
            return out;
        }
    } catch (...) {
        return {};
    }
    return {};
}

std::optional<std::vector<std::string>> load_safetensors_index_shards(const fs::path& index_path) {
    if (!is_valid_file(index_path)) return std::nullopt;
    try {
        std::ifstream ifs(index_path);
        nlohmann::json j;
        ifs >> j;

        if (!j.contains("weight_map") || !j["weight_map"].is_object()) {
            return std::nullopt;
        }

        const auto& weight_map = j["weight_map"];
        std::unordered_set<std::string> shard_set;
        for (auto it = weight_map.begin(); it != weight_map.end(); ++it) {
            if (!it.value().is_string()) continue;
            shard_set.insert(it.value().get<std::string>());
        }
        std::vector<std::string> shards(shard_set.begin(), shard_set.end());
        std::sort(shards.begin(), shards.end());
        return shards;
    } catch (...) {
        return std::nullopt;
    }
}

bool validate_safetensors_index_shards(const fs::path& model_dir, const fs::path& index_path) {
    auto shards = load_safetensors_index_shards(index_path);
    if (!shards) return false;

    // Empty weight_map is allowed (e.g., placeholder index for tests).
    for (const auto& shard : *shards) {
        const auto shard_path = model_dir / shard;
        if (!is_valid_file(shard_path)) {
            spdlog::warn("ModelStorage: missing safetensors shard: {}", shard_path.string());
            return false;
        }
    }
    return true;
}

std::optional<nlohmann::json> build_safetensors_metadata(const fs::path& model_dir, const fs::path& primary) {
    nlohmann::json st;
    st["index"] = primary.filename().string();

    if (is_safetensors_index_file(primary)) {
        auto shards = load_safetensors_index_shards(primary);
        if (!shards) return std::nullopt;
        st["shards"] = *shards;
    } else {
        st["shards"] = nlohmann::json::array({primary.filename().string()});
    }

    nlohmann::json meta;
    meta["safetensors"] = st;
    return meta;
}

std::string normalize_architecture_name(const std::string& value) {
    std::string lower = value;
    std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });

    std::string compact;
    compact.reserve(lower.size());
    for (char c : lower) {
        if (std::isalnum(static_cast<unsigned char>(c))) {
            compact.push_back(c);
        }
    }

    // Normalize architecture family names
    if (compact.find("qwen") != std::string::npos) return "qwen";
    if (compact.find("llama") != std::string::npos) return "llama";
    if (compact.find("mistral") != std::string::npos) return "mistral";
    if (compact.find("gemma") != std::string::npos) return "gemma";
    if (compact.find("phi") != std::string::npos) return "phi";
    if (compact.find("nemotron") != std::string::npos) return "nemotron";
    if (compact.find("deepseek") != std::string::npos) return "deepseek";
    if (compact.find("gptoss") != std::string::npos) return "gptoss";
    if (compact.find("granite") != std::string::npos) return "granite";
    if (compact.find("smollm") != std::string::npos) return "smollm";
    if (compact.find("kimi") != std::string::npos) return "kimi";
    if (compact.find("moondream") != std::string::npos) return "moondream";
    if (compact.find("snowflake") != std::string::npos) return "snowflake";
    if (compact.find("nomic") != std::string::npos) return "nomic";
    if (compact.find("mxbai") != std::string::npos) return "mxbai";
    if (compact.find("minilm") != std::string::npos) return "minilm";
    if (compact.find("devstral") != std::string::npos) return "devstral";
    if (compact.find("magistral") != std::string::npos) return "magistral";

    return compact.empty() ? lower : compact;
}

std::vector<std::string> extract_architectures_from_config(const nlohmann::json& j) {
    std::vector<std::string> out;
    if (!j.contains("architectures") || !j["architectures"].is_array()) return out;

    for (const auto& a : j["architectures"]) {
        if (!a.is_string()) continue;
        auto normalized = normalize_architecture_name(a.get<std::string>());
        if (normalized.empty()) continue;
        if (std::find(out.begin(), out.end(), normalized) == out.end()) {
            out.push_back(std::move(normalized));
        }
    }
    return out;
}

std::optional<std::string> detect_runtime_from_config(const fs::path& model_dir,
                                                      std::vector<std::string>* architectures) {
    const auto cfg_path = model_dir / "config.json";
    if (!fs::exists(cfg_path)) return std::nullopt;
    try {
        std::ifstream ifs(cfg_path);
        nlohmann::json j;
        ifs >> j;

        if (j.contains("architectures") && j["architectures"].is_array()) {
            if (architectures) {
                *architectures = extract_architectures_from_config(j);
            }
        }
    } catch (...) {
        return std::nullopt;
    }
    // SPEC-69549000: safetensors format models use safetensors_cpp engine
    return "safetensors_cpp";
}

std::string to_lower_ascii(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}

std::string normalize_architecture_label(const std::string& raw) {
    std::string lower = to_lower_ascii(raw);
    std::string out;
    out.reserve(lower.size());
    char prev = '\0';
    for (char c : lower) {
        if (std::isalnum(static_cast<unsigned char>(c))) {
            out.push_back(c);
            prev = c;
            continue;
        }
        if ((c == '_' || c == '-') && prev != '_') {
            out.push_back('_');
            prev = '_';
        }
    }
    if (!out.empty() && out.back() == '_') {
        out.pop_back();
    }
    return out;
}

std::string normalize_architecture_class(const std::string& raw) {
    std::string lower = to_lower_ascii(raw);
    auto pos = lower.find("for");
    if (pos != std::string::npos && pos > 0) {
        lower = lower.substr(0, pos);
    }
    return normalize_architecture_label(lower);
}

std::vector<std::string> load_architectures_from_config(const fs::path& model_dir) {
    const auto cfg_path = model_dir / "config.json";
    if (!fs::exists(cfg_path)) return {};
    try {
        std::ifstream ifs(cfg_path);
        nlohmann::json j;
        ifs >> j;

        std::vector<std::string> out;
        std::unordered_set<std::string> seen;
        auto add = [&](const std::string& raw, bool is_class_name) {
            const auto norm = is_class_name
                                  ? normalize_architecture_class(raw)
                                  : normalize_architecture_label(raw);
            if (norm.empty()) return;
            if (seen.insert(norm).second) {
                out.push_back(norm);
            }
        };

        if (j.contains("model_type") && j["model_type"].is_string()) {
            add(j["model_type"].get<std::string>(), false);
        }
        if (j.contains("architectures") && j["architectures"].is_array()) {
            for (const auto& a : j["architectures"]) {
                if (!a.is_string()) continue;
                add(a.get<std::string>(), true);
            }
        }
        return out;
    } catch (...) {
        return {};
    }
}

std::vector<std::string> capabilities_for_runtime(const std::string& runtime) {
    if (runtime == "llama_cpp") {
        return {"text", "embeddings"};
    }
    // SPEC-69549000: safetensors.cpp engine capabilities
    // Note: safetensors models are primarily for text generation (LlamaForCausalLM, etc.)
    // Embeddings require specific embedding models, not generic text models
    if (runtime == "safetensors_cpp") {
        return {"text"};
    }
    if (runtime == "whisper_cpp") {
        return {"asr"};
    }
    if (runtime == "onnx_runtime") {
        return {"tts"};
    }
    if (runtime == "stable_diffusion") {
        return {"image"};
    }
    return {};
}

std::optional<fs::path> resolve_safetensors_primary_in_dir(const fs::path& model_dir) {
    if (!has_required_safetensors_metadata(model_dir)) return std::nullopt;

    std::vector<fs::path> index_files;
    std::vector<fs::path> safetensors_files;

    std::error_code ec;
    for (const auto& entry : fs::directory_iterator(model_dir, ec)) {
        if (ec) break;
        // SPEC-69549000: シンボリックリンクもサポート（HuggingFaceキャッシュからのリンク対応）
        if (!is_regular_or_symlink_file(entry.path())) continue;

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
        if (c == '/' || c == '\\') {
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
    auto parsed = parse_model_name_with_quantization(model_name);
    const std::string& base = parsed ? parsed->base : model_name;
    return sanitizeModelId(base);
}

std::string ModelStorage::dirNameToModel(const std::string& dir_name) {
    // 一貫性のため、ディレクトリ名もサニタイズして小文字に正規化
    return sanitizeModelId(dir_name);
}

std::optional<ParsedModelName> ModelStorage::parseModelName(const std::string& model_name) {
    return parse_model_name_with_quantization(model_name);
}

std::string ModelStorage::resolveGguf(const std::string& model_name) const {
    auto parsed = parse_model_name_with_quantization(model_name);
    if (!parsed) return "";
    const std::string dir_name = modelNameToDir(parsed->base);
    const auto model_dir = fs::path(models_dir_) / dir_name;
    auto manifest_formats = load_manifest_formats(model_dir);
    if (!manifest_formats.empty()) {
        if (std::find(manifest_formats.begin(), manifest_formats.end(), "gguf") == manifest_formats.end()) {
            spdlog::debug("ModelStorage::resolveGguf: manifest formats exclude gguf, skip");
            return "";
        }
    }
    const auto gguf_path = resolve_gguf_path(
        model_dir,
        parsed->base,
        parsed->quantization.value_or(""));
    spdlog::debug("ModelStorage::resolveGguf: model={}, dir={}, path={}, exists={}",
        model_name, dir_name, gguf_path ? gguf_path->string() : "", gguf_path.has_value());

    if (gguf_path) return gguf_path->string();

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

        const auto manifest_formats = load_manifest_formats(model_dir);

        // GGUF
        const auto gguf_path = model_dir / "model.gguf";
        if (!manifest_formats.empty()) {
            bool resolved = false;
            for (const auto& fmt : manifest_formats) {
                if (fmt == "gguf") {
                    if (is_valid_file(gguf_path)) {
                        ModelInfo info;
                        info.name = dirNameToModel(relative.string());
                        info.format = "gguf";
                        info.primary_path = gguf_path.string();
                        info.valid = true;
                        out.push_back(std::move(info));
                        resolved = true;
                        break;
                    }
                } else if (fmt == "safetensors") {
                    if (auto primary = resolve_safetensors_primary_in_dir(model_dir)) {
                        ModelInfo info;
                        info.name = dirNameToModel(relative.string());
                        info.format = "safetensors";
                        info.primary_path = primary->string();
                        info.valid = true;
                        out.push_back(std::move(info));
                        resolved = true;
                        break;
                    }
                }
            }
            if (resolved) continue;
            continue;
        }

        if (is_valid_file(gguf_path)) {
            ModelInfo info;
            info.name = dirNameToModel(relative.string());
            info.format = "gguf";
            info.primary_path = gguf_path.string();
            info.valid = true;
            out.push_back(std::move(info));
            continue;
        }

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
            desc.capabilities = capabilities_for_runtime(desc.runtime);
            apply_manifest_quantization(desc, fs::path(desc.model_dir));
            out.push_back(std::move(desc));
            continue;
        }

        if (info.format == "safetensors") {
            std::vector<std::string> architectures;
            auto rt = detect_runtime_from_config(fs::path(desc.model_dir), &architectures);
            if (!rt) {
                continue;
            }
            desc.runtime = *rt;
            desc.architectures = std::move(architectures);
            desc.capabilities = capabilities_for_runtime(desc.runtime);
            desc.architectures = load_architectures_from_config(fs::path(desc.model_dir));
            if (auto meta = build_safetensors_metadata(fs::path(desc.model_dir), fs::path(desc.primary_path))) {
                desc.metadata = std::move(*meta);
            }
            apply_manifest_quantization(desc, fs::path(desc.model_dir));
            out.push_back(std::move(desc));
            continue;
        }
    }
    return out;
}

std::optional<ModelDescriptor> ModelStorage::resolveDescriptor(const std::string& model_name) const {
    auto parsed = parse_model_name_with_quantization(model_name);
    if (!parsed) return std::nullopt;
    const std::string dir_name = modelNameToDir(parsed->base);
    const auto model_dir = fs::path(models_dir_) / dir_name;

    const auto manifest_formats = load_manifest_formats(model_dir);
    if (!manifest_formats.empty()) {
        if (parsed->quantization.has_value()) {
            if (std::find(manifest_formats.begin(), manifest_formats.end(), "gguf") == manifest_formats.end()) {
                return std::nullopt;
            }
            if (auto gguf_path = resolve_gguf_path(model_dir, parsed->base, *parsed->quantization)) {
                ModelDescriptor desc;
                desc.name = model_name;
                desc.runtime = "llama_cpp";
                desc.format = "gguf";
                desc.primary_path = gguf_path->string();
                desc.model_dir = model_dir.string();
                desc.capabilities = capabilities_for_runtime(desc.runtime);
                apply_manifest_quantization(desc, model_dir);
                apply_quantization_request(desc, parsed->quantization);
                return desc;
            }
            return std::nullopt;
        }

        for (const auto& fmt : manifest_formats) {
            if (fmt == "gguf") {
                if (auto gguf_path = resolve_gguf_path(model_dir, parsed->base, "")) {
                    ModelDescriptor desc;
                    desc.name = model_name;
                    desc.runtime = "llama_cpp";
                    desc.format = "gguf";
                    desc.primary_path = gguf_path->string();
                    desc.model_dir = model_dir.string();
                    desc.capabilities = capabilities_for_runtime(desc.runtime);
                    apply_manifest_quantization(desc, model_dir);
                    apply_quantization_request(desc, parsed->quantization);
                    return desc;
                }
            } else if (fmt == "safetensors") {
                if (auto primary = resolve_safetensors_primary_in_dir(model_dir)) {
                    std::vector<std::string> architectures;
                    auto rt = detect_runtime_from_config(model_dir, &architectures);
                    if (!rt) return std::nullopt;
                    ModelDescriptor desc;
                    desc.name = model_name;
                    desc.runtime = *rt;
                    desc.format = "safetensors";
                    desc.primary_path = primary->string();
                    desc.model_dir = model_dir.string();
                    desc.architectures = std::move(architectures);
                    desc.capabilities = capabilities_for_runtime(desc.runtime);
                    if (auto meta = build_safetensors_metadata(model_dir, *primary)) {
                        desc.metadata = std::move(*meta);
                    }
                    return desc;
                }
            }
        }
        return std::nullopt;
    }

    if (auto gguf_path = resolve_gguf_path(model_dir, parsed->base, parsed->quantization.value_or(""))) {
        ModelDescriptor desc;
        desc.name = model_name;
        desc.runtime = "llama_cpp";
        desc.format = "gguf";
        desc.primary_path = gguf_path->string();
        desc.model_dir = model_dir.string();
        desc.capabilities = capabilities_for_runtime(desc.runtime);
        apply_manifest_quantization(desc, model_dir);
        apply_quantization_request(desc, parsed->quantization);
        return desc;
    }

    if (!parsed->quantization.has_value()) {
        if (auto primary = resolve_safetensors_primary_in_dir(model_dir)) {
            std::vector<std::string> architectures;
            auto rt = detect_runtime_from_config(model_dir, &architectures);
            if (!rt) return std::nullopt;
            ModelDescriptor desc;
            desc.name = model_name;
            desc.runtime = *rt;
            desc.format = "safetensors";
            desc.primary_path = primary->string();
            desc.model_dir = model_dir.string();
            desc.architectures = std::move(architectures);
            desc.capabilities = capabilities_for_runtime(desc.runtime);
            if (auto meta = build_safetensors_metadata(model_dir, *primary)) {
                desc.metadata = std::move(*meta);
            }
            return desc;
        }
    }

    return std::nullopt;
}

bool ModelStorage::validateModel(const std::string& model_name) const {
    auto parsed = parse_model_name_with_quantization(model_name);
    if (!parsed) return false;
    const std::string dir_name = modelNameToDir(parsed->base);
    const auto model_dir = fs::path(models_dir_) / dir_name;
    auto manifest_formats = load_manifest_formats(model_dir);
    if (!manifest_formats.empty()) {
        if (parsed->quantization.has_value()) {
            if (std::find(manifest_formats.begin(), manifest_formats.end(), "gguf") == manifest_formats.end()) {
                return false;
            }
            return resolve_gguf_path(model_dir, parsed->base, *parsed->quantization).has_value();
        }

        for (const auto& fmt : manifest_formats) {
            if (fmt == "gguf") {
                if (resolve_gguf_path(model_dir, parsed->base, "").has_value()) return true;
            } else if (fmt == "safetensors") {
                if (resolve_safetensors_primary_in_dir(model_dir).has_value()) return true;
            }
        }
        return false;
    }
    if (resolve_gguf_path(model_dir, parsed->base, parsed->quantization.value_or("")).has_value()) return true;
    if (!parsed->quantization.has_value()) {
        return resolve_safetensors_primary_in_dir(model_dir).has_value();
    }
    return false;
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

}  // namespace xllm
