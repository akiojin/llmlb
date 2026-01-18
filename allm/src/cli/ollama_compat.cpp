#include "cli/ollama_compat.h"
#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <cstdlib>

namespace fs = std::filesystem;

namespace allm {
namespace cli {

OllamaCompat::OllamaCompat(const std::string& ollama_dir) {
    if (ollama_dir.empty()) {
        ollama_dir_ = getDefaultOllamaDir();
    } else {
        ollama_dir_ = ollama_dir;
    }
}

OllamaCompat::~OllamaCompat() = default;

bool OllamaCompat::isAvailable() const {
    return fs::exists(ollama_dir_) && fs::is_directory(ollama_dir_);
}

std::vector<OllamaModelInfo> OllamaCompat::listModels() const {
    std::vector<OllamaModelInfo> models;

    if (!isAvailable()) {
        return models;
    }

    // Look in manifests directory: ~/.ollama/models/manifests/registry.ollama.ai/library/
    fs::path manifests_dir = fs::path(ollama_dir_) / "manifests" / "registry.ollama.ai" / "library";

    if (!fs::exists(manifests_dir)) {
        return models;
    }

    // Iterate through model directories
    for (const auto& model_entry : fs::directory_iterator(manifests_dir)) {
        if (!model_entry.is_directory()) {
            continue;
        }

        std::string model_name = model_entry.path().filename().string();

        // Iterate through tag files
        for (const auto& tag_entry : fs::directory_iterator(model_entry.path())) {
            if (!tag_entry.is_regular_file()) {
                continue;
            }

            std::string tag = tag_entry.path().filename().string();
            std::string full_name = model_name + ":" + tag;

            auto info = parseManifest(tag_entry.path().string(), full_name);
            if (info) {
                models.push_back(*info);
            }
        }
    }

    return models;
}

std::optional<OllamaModelInfo> OllamaCompat::getModel(const std::string& name) const {
    if (!isAvailable()) {
        return std::nullopt;
    }

    // Parse model name and tag
    std::string model_name = name;
    std::string tag = "latest";

    size_t colon_pos = name.find(':');
    if (colon_pos != std::string::npos) {
        model_name = name.substr(0, colon_pos);
        tag = name.substr(colon_pos + 1);
    }

    // Build manifest path
    fs::path manifest_path = fs::path(ollama_dir_) / "manifests" / "registry.ollama.ai" / "library" / model_name / tag;

    if (!fs::exists(manifest_path)) {
        return std::nullopt;
    }

    return parseManifest(manifest_path.string(), name);
}

std::string OllamaCompat::resolveBlobPath(const std::string& name) const {
    auto info = getModel(name);
    if (info) {
        return info->blob_path;
    }
    return "";
}

bool OllamaCompat::hasOllamaPrefix(const std::string& name) {
    return name.length() > 7 && name.substr(0, 7) == "ollama:";
}

std::string OllamaCompat::stripOllamaPrefix(const std::string& name) {
    if (hasOllamaPrefix(name)) {
        return name.substr(7);
    }
    return name;
}

std::optional<OllamaModelInfo> OllamaCompat::parseManifest(
    const std::string& manifest_path,
    const std::string& model_name
) const {
    std::ifstream file(manifest_path);
    if (!file) {
        return std::nullopt;
    }

    try {
        nlohmann::json manifest = nlohmann::json::parse(file);

        // Find the model layer (mediaType contains "model")
        std::string blob_digest;
        uint64_t blob_size = 0;

        if (manifest.contains("layers") && manifest["layers"].is_array()) {
            for (const auto& layer : manifest["layers"]) {
                std::string media_type = layer.value("mediaType", "");
                if (media_type.find("model") != std::string::npos) {
                    blob_digest = layer.value("digest", "");
                    blob_size = layer.value("size", 0);
                    break;
                }
            }
        }

        if (blob_digest.empty()) {
            return std::nullopt;
        }

        // Find blob file
        std::string blob_path = findBlob(blob_digest);
        if (blob_path.empty()) {
            return std::nullopt;
        }

        OllamaModelInfo info;
        info.name = model_name;
        info.manifest_path = manifest_path;
        info.blob_digest = blob_digest;
        info.blob_path = blob_path;
        info.size_bytes = blob_size;
        info.readonly = true;

        return info;

    } catch (const nlohmann::json::exception&) {
        return std::nullopt;
    }
}

std::string OllamaCompat::findBlob(const std::string& digest) const {
    // Remove sha256: prefix if present
    std::string clean_digest = digest;
    if (clean_digest.length() > 7 && clean_digest.substr(0, 7) == "sha256:") {
        clean_digest = clean_digest.substr(7);
    }

    // Build blob path: ~/.ollama/models/blobs/sha256-<digest>
    fs::path blob_path = fs::path(ollama_dir_) / "blobs" / ("sha256-" + clean_digest);

    if (fs::exists(blob_path)) {
        return blob_path.string();
    }

    return "";
}

std::string OllamaCompat::getDefaultOllamaDir() {
    // Check OLLAMA_MODELS environment variable first
    const char* env_dir = std::getenv("OLLAMA_MODELS");
    if (env_dir && fs::exists(env_dir)) {
        return env_dir;
    }

    // Default to ~/.ollama/models
    const char* home = std::getenv("HOME");
    if (home) {
        return std::string(home) + "/.ollama/models";
    }

    return "";
}

}  // namespace cli
}  // namespace allm
