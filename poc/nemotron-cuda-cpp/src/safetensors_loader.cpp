#include "safetensors_loader.h"

#define SAFETENSORS_CPP_IMPLEMENTATION
#include "safetensors.hh"

#include <fstream>
#include <filesystem>

namespace nemotron {

namespace fs = std::filesystem;

struct SafetensorsLoader::MmapHandle {
    safetensors::safetensors_t st;
    std::string path;

    ~MmapHandle() {
        // safetensors.hh handles unmapping internally
    }
};

SafetensorsLoader::~SafetensorsLoader() = default;

SafetensorsLoader::SafetensorsLoader(SafetensorsLoader&&) noexcept = default;
SafetensorsLoader& SafetensorsLoader::operator=(SafetensorsLoader&&) noexcept = default;

void SafetensorsLoader::loadFile(const std::string& path) {
    LOG_INFO("Loading safetensors file: " << path);

    auto handle = std::make_unique<MmapHandle>();
    handle->path = path;

    std::string warn, err;
    bool ok = safetensors::mmap_from_file(path, &handle->st, &warn, &err);

    if (!warn.empty()) {
        LOG_WARN("safetensors warning: " << warn);
    }
    if (!ok) {
        throw FileError("Failed to load safetensors: " + path + " - " + err);
    }

    if (!safetensors::validate_data_offsets(handle->st, err)) {
        throw FileError("Invalid safetensors data offsets: " + err);
    }

    // Extract tensor info
    const auto& keys = handle->st.tensors.keys();
    for (const auto& name : keys) {
        safetensors::tensor_t tensor;
        if (!handle->st.tensors.at(name, &tensor)) {
            continue;
        }

        TensorInfo info;
        info.name = name;
        info.data = tensor.data;
        info.data_size = tensor.data_bytes;

        // Convert dtype
        switch (tensor.dtype) {
            case safetensors::kFLOAT16: info.dtype = DType::F16; break;
            case safetensors::kBFLOAT16: info.dtype = DType::BF16; break;
            case safetensors::kFLOAT32: info.dtype = DType::F32; break;
            case safetensors::kINT32: info.dtype = DType::I32; break;
            case safetensors::kINT64: info.dtype = DType::I64; break;
            default: info.dtype = DType::Unknown; break;
        }

        // Copy shape
        info.shape.assign(tensor.shape.begin(), tensor.shape.end());

        tensors_[name] = std::move(info);
        total_size_ += tensor.data_bytes;
    }

    mmap_handles_.push_back(std::move(handle));
    LOG_INFO("  Loaded " << keys.size() << " tensors, total size: "
             << (total_size_ / (1024 * 1024)) << " MB");
}

void SafetensorsLoader::loadSharded(const std::string& model_dir) {
    std::string index_path = model_dir + "/model.safetensors.index.json";

    if (!fs::exists(index_path)) {
        throw FileError("Shard index not found: " + index_path);
    }

    LOG_INFO("Loading sharded model from: " << model_dir);

    // Parse index.json to find shards
    std::ifstream file(index_path);
    if (!file.is_open()) {
        throw FileError("Cannot open index file: " + index_path);
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                        std::istreambuf_iterator<char>());

    // Simple parsing: find "weight_map" and extract shard filenames
    std::set<std::string> shard_files;
    size_t pos = content.find("\"weight_map\"");
    if (pos != std::string::npos) {
        pos = content.find('{', pos);
        size_t end = content.find('}', pos);
        if (pos != std::string::npos && end != std::string::npos) {
            std::string weight_map = content.substr(pos, end - pos + 1);
            // Extract .safetensors filenames
            size_t search_pos = 0;
            while ((search_pos = weight_map.find(".safetensors", search_pos)) != std::string::npos) {
                // Find the start of the filename
                size_t quote_end = search_pos + 12;  // length of ".safetensors"
                size_t quote_start = weight_map.rfind('"', search_pos);
                if (quote_start != std::string::npos) {
                    std::string filename = weight_map.substr(quote_start + 1, quote_end - quote_start - 1);
                    shard_files.insert(filename);
                }
                search_pos = quote_end;
            }
        }
    }

    if (shard_files.empty()) {
        throw FileError("No shard files found in index: " + index_path);
    }

    LOG_INFO("  Found " << shard_files.size() << " shards");

    // Load each shard
    for (const auto& shard : shard_files) {
        std::string shard_path = model_dir + "/" + shard;
        if (fs::exists(shard_path)) {
            loadFile(shard_path);
        } else {
            LOG_WARN("Shard file not found: " << shard_path);
        }
    }
}

void SafetensorsLoader::loadModel(const std::string& model_dir) {
    // Check for sharded model first
    std::string index_path = model_dir + "/model.safetensors.index.json";
    if (fs::exists(index_path)) {
        loadSharded(model_dir);
        return;
    }

    // Check for single file
    std::string single_path = model_dir + "/model.safetensors";
    if (fs::exists(single_path)) {
        loadFile(single_path);
        return;
    }

    // Try to find any .safetensors file
    for (const auto& entry : fs::directory_iterator(model_dir)) {
        if (entry.path().extension() == ".safetensors") {
            loadFile(entry.path().string());
            return;
        }
    }

    throw FileError("No safetensors files found in: " + model_dir);
}

const TensorInfo* SafetensorsLoader::getTensor(const std::string& name) const {
    auto it = tensors_.find(name);
    if (it != tensors_.end()) {
        return &it->second;
    }
    return nullptr;
}

bool SafetensorsLoader::hasTensor(const std::string& name) const {
    return tensors_.find(name) != tensors_.end();
}

std::vector<std::string> SafetensorsLoader::getTensorNames() const {
    std::vector<std::string> names;
    names.reserve(tensors_.size());
    for (const auto& [name, _] : tensors_) {
        names.push_back(name);
    }
    return names;
}

const char* dtypeToString(DType dtype) {
    switch (dtype) {
        case DType::F16: return "F16";
        case DType::BF16: return "BF16";
        case DType::F32: return "F32";
        case DType::I32: return "I32";
        case DType::I64: return "I64";
        default: return "Unknown";
    }
}

size_t dtypeSize(DType dtype) {
    switch (dtype) {
        case DType::F16:
        case DType::BF16: return 2;
        case DType::F32:
        case DType::I32: return 4;
        case DType::I64: return 8;
        default: return 0;
    }
}

}  // namespace nemotron
