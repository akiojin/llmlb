#include "core/nemotron_engine.h"

#include <filesystem>
#include <fstream>
#include <optional>
#include <stdexcept>

#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#define SAFETENSORS_CPP_IMPLEMENTATION
#include "safetensors.hh"

namespace fs = std::filesystem;

namespace llm_node {

namespace {
constexpr const char* kKnownTensorName = "backbone.layers.1.mixer.experts.0.down_proj.weight";

bool is_index_file(const fs::path& path) {
    const std::string filename = path.filename().string();
    return filename.find(".safetensors.index.json") != std::string::npos;
}

std::optional<nlohmann::json> load_json(const fs::path& path, std::string& err) {
    std::ifstream ifs(path);
    if (!ifs) {
        err = "Failed to open index file: " + path.string();
        return std::nullopt;
    }
    try {
        nlohmann::json j;
        ifs >> j;
        return j;
    } catch (const std::exception& e) {
        err = std::string("Failed to parse JSON: ") + e.what();
        return std::nullopt;
    }
}

std::optional<std::string> find_shard_for_tensor(const nlohmann::json& index,
                                                 const std::string& tensor_name,
                                                 std::string& err) {
    if (!index.is_object()) {
        err = "Index JSON is not an object";
        return std::nullopt;
    }
    if (!index.contains("weight_map") || !index["weight_map"].is_object()) {
        err = "Index JSON missing weight_map";
        return std::nullopt;
    }
    const auto& weight_map = index["weight_map"];
    if (!weight_map.contains(tensor_name) || !weight_map[tensor_name].is_string()) {
        err = "Tensor not found in weight_map: " + tensor_name;
        return std::nullopt;
    }
    return weight_map[tensor_name].get<std::string>();
}

ModelLoadResult validate_safetensors_file(const fs::path& path, const std::string& expected_tensor) {
    ModelLoadResult result;
    if (!fs::exists(path)) {
        result.error_message = "Safetensors file not found: " + path.string();
        return result;
    }

    safetensors::safetensors_t st;
    std::string warn;
    std::string err;
    if (!safetensors::mmap_from_file(path.string(), &st, &warn, &err)) {
        result.error_message = err.empty() ? "Failed to mmap safetensors file" : err;
        return result;
    }

    if (!warn.empty()) {
        spdlog::warn("NemotronEngine: safetensors warning: {}", warn);
    }

    std::string validate_err;
    if (!safetensors::validate_data_offsets(st, validate_err)) {
        result.error_message = validate_err.empty() ? "Invalid data_offsets in safetensors" : validate_err;
        return result;
    }

    if (!expected_tensor.empty() && !st.tensors.count(expected_tensor)) {
        result.error_message = "Expected tensor not found: " + expected_tensor;
        return result;
    }

    result.success = true;
    return result;
}
}  // namespace

ModelLoadResult NemotronEngine::loadModel(const ModelDescriptor& descriptor) {
    ModelLoadResult result;
    if (!descriptor.format.empty() && descriptor.format != "safetensors") {
        result.error_message = "Nemotron engine supports safetensors only";
        return result;
    }
    if (descriptor.primary_path.empty()) {
        result.error_message = "Nemotron primary path is empty";
        return result;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        if (loaded_.count(descriptor.primary_path) != 0) {
            result.success = true;
            return result;
        }
    }

    fs::path primary(descriptor.primary_path);
    if (!fs::exists(primary)) {
        result.error_message = "Primary path not found: " + primary.string();
        return result;
    }

    if (is_index_file(primary)) {
        std::string err;
        auto index = load_json(primary, err);
        if (!index) {
            result.error_message = err;
            return result;
        }
        auto shard = find_shard_for_tensor(*index, kKnownTensorName, err);
        if (!shard) {
            result.error_message = err;
            return result;
        }
        fs::path shard_path(*shard);
        if (!shard_path.is_absolute()) {
            shard_path = primary.parent_path() / shard_path;
        }
        result = validate_safetensors_file(shard_path, kKnownTensorName);
    } else {
        result = validate_safetensors_file(primary, kKnownTensorName);
    }

    if (result.success) {
        std::lock_guard<std::mutex> lock(mutex_);
        loaded_.insert(descriptor.primary_path);
    }

    return result;
}

std::string NemotronEngine::generateChat(
    const std::vector<ChatMessage>&,
    const ModelDescriptor&,
    const InferenceParams&) const {
    throw std::runtime_error("Nemotron engine does not support text generation yet");
}

std::string NemotronEngine::generateCompletion(
    const std::string&,
    const ModelDescriptor&,
    const InferenceParams&) const {
    throw std::runtime_error("Nemotron engine does not support text generation yet");
}

std::vector<std::string> NemotronEngine::generateChatStream(
    const std::vector<ChatMessage>&,
    const ModelDescriptor&,
    const InferenceParams&,
    const std::function<void(const std::string&)>&) const {
    throw std::runtime_error("Nemotron engine does not support text generation yet");
}

std::vector<std::vector<float>> NemotronEngine::generateEmbeddings(
    const std::vector<std::string>&,
    const ModelDescriptor&) const {
    throw std::runtime_error("Nemotron engine does not support embeddings yet");
}

size_t NemotronEngine::getModelMaxContext(const ModelDescriptor&) const {
    return 0;
}

}  // namespace llm_node
