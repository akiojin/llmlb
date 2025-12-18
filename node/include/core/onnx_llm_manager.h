#pragma once

#include <chrono>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

#ifdef USE_ONNX_RUNTIME
#include <onnxruntime_cxx_api.h>
#endif

namespace llm_node {

class OnnxLlmManager {
public:
    explicit OnnxLlmManager(std::string models_dir);
    ~OnnxLlmManager();

    OnnxLlmManager(const OnnxLlmManager&) = delete;
    OnnxLlmManager& operator=(const OnnxLlmManager&) = delete;

    /// Load a model session from path (relative to models_dir or absolute).
    bool loadModel(const std::string& model_path);

    /// Check if a model is loaded.
    bool isLoaded(const std::string& model_path) const;

    /// Load model if needed (on-demand loading).
    bool loadModelIfNeeded(const std::string& model_path);

    /// Unload a specific model.
    bool unloadModel(const std::string& model_path);

    /// List loaded models (canonical paths).
    std::vector<std::string> getLoadedModels() const;

    /// Number of loaded models.
    size_t loadedCount() const;

    /// Memory usage estimate (sum of model file sizes).
    size_t memoryUsageBytes() const;

    /// Idle timeout configuration.
    void setIdleTimeout(std::chrono::milliseconds timeout);
    std::chrono::milliseconds getIdleTimeout() const;

    /// Unload models idle longer than timeout.
    size_t unloadIdleModels();

    /// Max loaded models configuration (0 = unlimited).
    void setMaxLoadedModels(size_t max_models);
    size_t getMaxLoadedModels() const;

    /// Check if more models can be loaded considering limits.
    bool canLoadMore() const;

    /// Max memory estimate limit (0 = unlimited).
    void setMaxMemoryBytes(size_t max_bytes);
    size_t getMaxMemoryBytes() const;

    /// Last access time for model (canonical path).
    std::optional<std::chrono::steady_clock::time_point> getLastAccessTime(
        const std::string& model_path) const;

    /// LRU: least recently used model (canonical path).
    std::optional<std::string> getLeastRecentlyUsedModel() const;

    /// Get loaded session (nullptr if not loaded or runtime unavailable).
    /// Lifetime is owned by this manager.
    Ort::Session* getSession(const std::string& model_path) const;

    /// Check if ONNX Runtime is available in this build.
    static bool isRuntimeAvailable();

private:
    std::string models_dir_;
    mutable std::mutex mutex_;

#ifdef USE_ONNX_RUNTIME
    Ort::Env env_;
    std::unordered_map<std::string, std::unique_ptr<Ort::Session>> loaded_models_;
#endif

    size_t memory_bytes_{0};

    std::unordered_map<std::string, std::chrono::steady_clock::time_point> last_access_;
    std::chrono::milliseconds idle_timeout_{std::chrono::minutes(5)};
    size_t max_loaded_models_{0};  // 0 = unlimited
    size_t max_memory_bytes_{0};   // 0 = unlimited

    std::string canonicalizePath(const std::string& path) const;
    void updateAccessTime(const std::string& canonical_path);
    bool canLoadMoreUnlocked() const;

#ifdef USE_ONNX_RUNTIME
    std::unique_ptr<Ort::Session> createSession(const std::string& canonical_path) const;
#endif
};

}  // namespace llm_node
