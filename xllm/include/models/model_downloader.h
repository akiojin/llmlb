#pragma once

#include <string>
#include <functional>
#include <chrono>
#include <mutex>

namespace xllm {

using ProgressCallback = std::function<void(size_t downloaded, size_t total)>;

class ModelDownloader {
public:
    ModelDownloader(std::string registry_base,
                    std::string models_dir,
                    std::chrono::milliseconds timeout = std::chrono::milliseconds(10000),
                    int max_retries = 2,
                    std::chrono::milliseconds backoff = std::chrono::milliseconds(200),
                    std::string api_key = {});

    // Fetch manifest JSON for a model id (e.g., gpt-oss-7b). Returns local manifest path.
    // filename_hint can be used to disambiguate a specific artifact in HuggingFace repos.
    std::string fetchManifest(const std::string& model_id, const std::string& filename_hint = {});

    // Download a blob by URL to the model directory. Reports progress if provided.
    // If expected_sha256 is provided, verify the downloaded file; on mismatch an empty string is returned.
    std::string downloadBlob(const std::string& blob_url, const std::string& filename, ProgressCallback cb = nullptr,
                             const std::string& expected_sha256 = "", const std::string& if_none_match = "");

    const std::string& getModelsDir() const { return models_dir_; }
    const std::string& getRegistryBase() const { return registry_base_; }
    std::string getLastError() const {
        std::lock_guard<std::mutex> lock(error_mutex_);
        return last_error_;
    }
    size_t getChunkSize() const { return chunk_size_; }
    size_t getMaxBytesPerSec() const { return max_bytes_per_sec_; }
    void setChunkSize(size_t v) { chunk_size_ = v; }
    void setMaxBytesPerSec(size_t v) { max_bytes_per_sec_ = v; }

private:
    std::string fetchHfManifest(const std::string& model_id, const std::string& filename_hint);

    std::string registry_base_;
    std::string models_dir_;
    std::chrono::milliseconds timeout_;
    int max_retries_;
    std::chrono::milliseconds backoff_;
    std::string api_key_;
    size_t max_bytes_per_sec_{0};
    size_t chunk_size_{4096};
    std::string log_source_;
    std::string last_error_;
    mutable std::mutex error_mutex_;
};

}  // namespace xllm
