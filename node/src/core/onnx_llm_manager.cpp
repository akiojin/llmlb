#include "core/onnx_llm_manager.h"

#include <spdlog/spdlog.h>
#include <algorithm>
#include <cctype>
#include <filesystem>

namespace fs = std::filesystem;

namespace llm_node {

namespace {

bool isHex(char c) {
    return std::isxdigit(static_cast<unsigned char>(c)) != 0;
}

bool isRuntimeBlobName(const std::string& filename) {
    // Backward compatibility: LLM runtime blob format "sha256-<hex...>"
    constexpr const char* kPrefix = "sha256-";
    if (filename.rfind(kPrefix, 0) != 0) return false;
    if (filename.size() <= std::char_traits<char>::length(kPrefix)) return false;
    for (size_t i = std::char_traits<char>::length(kPrefix); i < filename.size(); ++i) {
        if (!isHex(filename[i])) return false;
    }
    return true;
}

bool hasAllowedExtension(const std::string& path) {
    const fs::path p(path);
    const auto ext = p.extension().string();
    if (ext == ".gguf" || ext == ".onnx") return true;
    // Allow blob-style files without extension.
    return isRuntimeBlobName(p.filename().string());
}

size_t fileSizeOrZero(const std::string& path) {
    std::error_code ec;
    const auto size = fs::file_size(path, ec);
    if (ec) return 0;
    return static_cast<size_t>(size);
}

}  // namespace

OnnxLlmManager::OnnxLlmManager(std::string models_dir)
    : models_dir_(std::move(models_dir))
#ifdef USE_ONNX_RUNTIME
    , env_(ORT_LOGGING_LEVEL_WARNING, "OnnxLlmManager")
#endif
{
    spdlog::info("OnnxLlmManager initialized with models dir: {}", models_dir_);
}

OnnxLlmManager::~OnnxLlmManager() {
#ifdef USE_ONNX_RUNTIME
    std::lock_guard<std::mutex> lock(mutex_);
    loaded_models_.clear();
    spdlog::info("OnnxLlmManager destroyed, all models unloaded");
#endif
}

bool OnnxLlmManager::isRuntimeAvailable() {
#ifdef USE_ONNX_RUNTIME
    return true;
#else
    return false;
#endif
}

std::string OnnxLlmManager::canonicalizePath(const std::string& path) const {
    try {
        const fs::path p(path);
        if (p.is_absolute()) {
            return fs::canonical(p).string();
        }
        return fs::canonical(fs::path(models_dir_) / p).string();
    } catch (const fs::filesystem_error&) {
        if (fs::path(path).is_absolute()) {
            return path;
        }
        return (fs::path(models_dir_) / path).string();
    }
}

void OnnxLlmManager::updateAccessTime(const std::string& canonical_path) {
    last_access_[canonical_path] = std::chrono::steady_clock::now();
}

bool OnnxLlmManager::canLoadMoreUnlocked() const {
    if (max_loaded_models_ != 0) {
#ifdef USE_ONNX_RUNTIME
        if (loaded_models_.size() >= max_loaded_models_) return false;
#else
        return false;
#endif
    }

    if (max_memory_bytes_ != 0 && memory_bytes_ >= max_memory_bytes_) {
        return false;
    }

    return true;
}

bool OnnxLlmManager::canLoadMore() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return canLoadMoreUnlocked();
}

#ifdef USE_ONNX_RUNTIME
std::unique_ptr<Ort::Session> OnnxLlmManager::createSession(const std::string& canonical_path) const {
    Ort::SessionOptions session_options;
    session_options.SetIntraOpNumThreads(4);
    session_options.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_ALL);

#if defined(__APPLE__)
    // Try CoreML first (if the runtime was built with CoreML EP). Failure should fall back to CPU EP.
    try {
        session_options.AppendExecutionProvider("CoreMLExecutionProvider");
        spdlog::info("ONNX Runtime: CoreML EP enabled");
    } catch (const Ort::Exception& e) {
        spdlog::debug("ONNX Runtime: CoreML EP not available: {}", e.what());
    }
#endif
    // Try XNNPACK (CPU acceleration) when available.
    try {
        session_options.AppendExecutionProvider("XnnpackExecutionProvider");
        spdlog::info("ONNX Runtime: XNNPACK EP enabled");
    } catch (const Ort::Exception& e) {
        spdlog::debug("ONNX Runtime: XNNPACK EP not available: {}", e.what());
    }

    return std::make_unique<Ort::Session>(env_, canonical_path.c_str(), session_options);
}
#endif

bool OnnxLlmManager::loadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);

    const std::string canonical_path = canonicalizePath(model_path);

    if (!hasAllowedExtension(canonical_path)) {
        spdlog::warn("Rejecting unsupported model file extension: {}", canonical_path);
        return false;
    }

#ifdef USE_ONNX_RUNTIME
    if (loaded_models_.find(canonical_path) != loaded_models_.end()) {
        updateAccessTime(canonical_path);
        return true;
    }

    if (!canLoadMoreUnlocked()) {
        spdlog::warn("Cannot load more models due to limits: loaded={}, mem={} bytes",
                     loaded_models_.size(), memory_bytes_);
        return false;
    }

    std::error_code ec;
    if (!fs::exists(canonical_path, ec) || !fs::is_regular_file(canonical_path, ec)) {
        spdlog::warn("Model file not found: {}", canonical_path);
        return false;
    }

    try {
        spdlog::info("Loading ONNX model: {}", canonical_path);
        auto session = createSession(canonical_path);
        loaded_models_[canonical_path] = std::move(session);
        memory_bytes_ += fileSizeOrZero(canonical_path);
        updateAccessTime(canonical_path);
        spdlog::info("ONNX model loaded successfully: {}", canonical_path);
        return true;
    } catch (const Ort::Exception& e) {
        spdlog::error("Failed to load ONNX model: {} - {}", canonical_path, e.what());
        return false;
    }
#else
    spdlog::warn("ONNX Runtime not available, cannot load model: {}", canonical_path);
    return false;
#endif
}

bool OnnxLlmManager::isLoaded(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
#ifdef USE_ONNX_RUNTIME
    const std::string canonical_path = canonicalizePath(model_path);
    return loaded_models_.find(canonical_path) != loaded_models_.end();
#else
    (void)model_path;
    return false;
#endif
}

bool OnnxLlmManager::loadModelIfNeeded(const std::string& model_path) {
    if (isLoaded(model_path)) {
        std::lock_guard<std::mutex> lock(mutex_);
        updateAccessTime(canonicalizePath(model_path));
        return true;
    }
    return loadModel(model_path);
}

bool OnnxLlmManager::unloadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);
#ifdef USE_ONNX_RUNTIME
    const std::string canonical_path = canonicalizePath(model_path);
    auto it = loaded_models_.find(canonical_path);
    if (it == loaded_models_.end()) return false;

    loaded_models_.erase(it);
    last_access_.erase(canonical_path);

    const size_t sz = fileSizeOrZero(canonical_path);
    if (sz <= memory_bytes_) memory_bytes_ -= sz;
    spdlog::info("ONNX model unloaded: {}", canonical_path);
    return true;
#else
    (void)model_path;
    return false;
#endif
}

std::vector<std::string> OnnxLlmManager::getLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<std::string> out;
#ifdef USE_ONNX_RUNTIME
    out.reserve(loaded_models_.size());
    for (const auto& [path, _] : loaded_models_) {
        out.push_back(path);
    }
#endif
    return out;
}

size_t OnnxLlmManager::loadedCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
#ifdef USE_ONNX_RUNTIME
    return loaded_models_.size();
#else
    return 0;
#endif
}

size_t OnnxLlmManager::memoryUsageBytes() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return memory_bytes_;
}

void OnnxLlmManager::setIdleTimeout(std::chrono::milliseconds timeout) {
    std::lock_guard<std::mutex> lock(mutex_);
    idle_timeout_ = timeout;
}

std::chrono::milliseconds OnnxLlmManager::getIdleTimeout() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return idle_timeout_;
}

size_t OnnxLlmManager::unloadIdleModels() {
    std::lock_guard<std::mutex> lock(mutex_);

    const auto now = std::chrono::steady_clock::now();
    std::vector<std::string> to_unload;
    for (const auto& [path, last_time] : last_access_) {
        const auto idle = std::chrono::duration_cast<std::chrono::milliseconds>(now - last_time);
        if (idle_timeout_.count() > 0 && idle >= idle_timeout_) {
            to_unload.push_back(path);
        }
    }

#ifdef USE_ONNX_RUNTIME
    for (const auto& path : to_unload) {
        auto it = loaded_models_.find(path);
        if (it != loaded_models_.end()) {
            loaded_models_.erase(it);
            last_access_.erase(path);
            const size_t sz = fileSizeOrZero(path);
            if (sz <= memory_bytes_) memory_bytes_ -= sz;
            spdlog::info("Unloaded idle ONNX model: {}", path);
        }
    }
#endif

    return to_unload.size();
}

void OnnxLlmManager::setMaxLoadedModels(size_t max_models) {
    std::lock_guard<std::mutex> lock(mutex_);
    max_loaded_models_ = max_models;
}

size_t OnnxLlmManager::getMaxLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return max_loaded_models_;
}

void OnnxLlmManager::setMaxMemoryBytes(size_t max_bytes) {
    std::lock_guard<std::mutex> lock(mutex_);
    max_memory_bytes_ = max_bytes;
}

size_t OnnxLlmManager::getMaxMemoryBytes() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return max_memory_bytes_;
}

std::optional<std::chrono::steady_clock::time_point> OnnxLlmManager::getLastAccessTime(
    const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    const std::string canonical_path = canonicalizePath(model_path);
    auto it = last_access_.find(canonical_path);
    if (it == last_access_.end()) return std::nullopt;
    return it->second;
}

std::optional<std::string> OnnxLlmManager::getLeastRecentlyUsedModel() const {
    std::lock_guard<std::mutex> lock(mutex_);
    if (last_access_.empty()) return std::nullopt;
    auto min_it = std::min_element(
        last_access_.begin(),
        last_access_.end(),
        [](const auto& a, const auto& b) { return a.second < b.second; });
    return min_it->first;
}

const Ort::Session* OnnxLlmManager::getSession(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
#ifdef USE_ONNX_RUNTIME
    const std::string canonical_path = canonicalizePath(model_path);
    auto it = loaded_models_.find(canonical_path);
    if (it == loaded_models_.end()) return nullptr;
    return it->second.get();
#else
    (void)model_path;
    return nullptr;
#endif
}

}  // namespace llm_node
