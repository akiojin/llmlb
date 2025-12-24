#pragma once

#include <string>
#include <vector>
#include <memory>
#include <unordered_map>
#include "config.h"

namespace nemotron {

// Tensor data type
enum class DType {
    F16,
    BF16,
    F32,
    I32,
    I64,
    Unknown
};

// Tensor metadata
struct TensorInfo {
    std::string name;
    DType dtype;
    Shape shape;
    size_t data_offset;  // Offset in mmap buffer
    size_t data_size;    // Size in bytes
    const void* data;    // Pointer to data (mmap'd)
};

// Safetensors file loader
class SafetensorsLoader {
public:
    SafetensorsLoader() = default;
    ~SafetensorsLoader();

    SafetensorsLoader(const SafetensorsLoader&) = delete;
    SafetensorsLoader& operator=(const SafetensorsLoader&) = delete;
    SafetensorsLoader(SafetensorsLoader&&) noexcept;
    SafetensorsLoader& operator=(SafetensorsLoader&&) noexcept;

    // Load single safetensors file
    void loadFile(const std::string& path);

    // Load sharded model (model.safetensors.index.json + shards)
    void loadSharded(const std::string& model_dir);

    // Load model from directory (auto-detect single/sharded)
    void loadModel(const std::string& model_dir);

    // Get tensor by name
    const TensorInfo* getTensor(const std::string& name) const;

    // Check if tensor exists
    bool hasTensor(const std::string& name) const;

    // Get all tensor names
    std::vector<std::string> getTensorNames() const;

    // Get total loaded size
    size_t getTotalSize() const { return total_size_; }

private:
    struct MmapHandle;
    std::vector<std::unique_ptr<MmapHandle>> mmap_handles_;
    std::unordered_map<std::string, TensorInfo> tensors_;
    size_t total_size_ = 0;

    void parseHeader(const char* data, size_t file_size, const std::string& path);
};

// Helper to convert dtype enum to string
const char* dtypeToString(DType dtype);

// Helper to get dtype size in bytes
size_t dtypeSize(DType dtype);

}  // namespace nemotron
