#include "core/whisper_manager.h"

#include <spdlog/spdlog.h>
#include <whisper.h>
#include <filesystem>
#include <algorithm>

namespace llm_node {

WhisperManager::WhisperManager(std::string models_dir)
    : models_dir_(std::move(models_dir)) {
    spdlog::info("WhisperManager initialized with models dir: {}", models_dir_);
}

WhisperManager::~WhisperManager() {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [path, ctx] : loaded_models_) {
        if (ctx) {
            whisper_free(ctx);
        }
    }
    loaded_models_.clear();
    spdlog::info("WhisperManager destroyed, all models unloaded");
}

std::string WhisperManager::canonicalizePath(const std::string& path) const {
    try {
        if (std::filesystem::path(path).is_absolute()) {
            return std::filesystem::canonical(path).string();
        }
        return std::filesystem::canonical(
            std::filesystem::path(models_dir_) / path).string();
    } catch (const std::filesystem::filesystem_error& e) {
        // ファイルが存在しない場合はそのままのパスを返す
        if (std::filesystem::path(path).is_absolute()) {
            return path;
        }
        return (std::filesystem::path(models_dir_) / path).string();
    }
}

void WhisperManager::updateAccessTime(const std::string& model_path) {
    last_access_[model_path] = std::chrono::steady_clock::now();
}

bool WhisperManager::loadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);

    std::string canonical_path = canonicalizePath(model_path);

    if (loaded_models_.find(canonical_path) != loaded_models_.end()) {
        spdlog::debug("Whisper model already loaded: {}", canonical_path);
        updateAccessTime(canonical_path);
        return true;
    }

    if (!canLoadMore()) {
        spdlog::warn("Cannot load more whisper models, max limit reached: {}", max_loaded_models_);
        return false;
    }

    spdlog::info("Loading whisper model: {}", canonical_path);

    whisper_context_params cparams = whisper_context_default_params();
    cparams.use_gpu = true;  // GPUが利用可能なら使用
    cparams.flash_attn = false;  // Avoid flash-attn assertion crashes on some Metal paths

    whisper_context* ctx = whisper_init_from_file_with_params(
        canonical_path.c_str(), cparams);

    if (!ctx) {
        spdlog::error("Failed to load whisper model: {}", canonical_path);
        return false;
    }

    loaded_models_[canonical_path] = ctx;
    updateAccessTime(canonical_path);

    spdlog::info("Whisper model loaded successfully: {}", canonical_path);
    return true;
}

bool WhisperManager::isLoaded(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);
    return loaded_models_.find(canonical_path) != loaded_models_.end();
}

whisper_context* WhisperManager::getContext(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);
    auto it = loaded_models_.find(canonical_path);
    if (it != loaded_models_.end()) {
        return it->second;
    }
    return nullptr;
}

whisper_full_params WhisperManager::createParams(const TranscriptionParams& params) const {
    whisper_full_params wparams = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);

    wparams.n_threads = params.max_threads;
    wparams.translate = params.translate;
    wparams.print_progress = false;
    wparams.print_special = false;
    wparams.print_realtime = false;
    wparams.print_timestamps = false;

    // 言語設定
    if (!params.language.empty() && params.language != "auto") {
        wparams.language = params.language.c_str();
        wparams.detect_language = false;
    } else {
        wparams.language = nullptr;
        wparams.detect_language = true;
    }

    return wparams;
}

TranscriptionResult WhisperManager::transcribe(
    const std::string& model_path,
    const std::vector<float>& audio_data,
    int sample_rate,
    const TranscriptionParams& params) {

    TranscriptionResult result;

    if (audio_data.empty()) {
        result.error = "Empty audio data";
        return result;
    }

    // サンプルレートの検証 (whisper.cppは16kHz固定)
    if (sample_rate != WHISPER_SAMPLE_RATE) {
        result.error = "Sample rate must be 16000 Hz, got " + std::to_string(sample_rate);
        return result;
    }

    std::string canonical_path = canonicalizePath(model_path);

    whisper_context* ctx = nullptr;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = loaded_models_.find(canonical_path);
        if (it == loaded_models_.end()) {
            result.error = "Model not loaded: " + canonical_path;
            return result;
        }
        ctx = it->second;
        updateAccessTime(canonical_path);
    }

    whisper_full_params wparams = createParams(params);

    spdlog::debug("Running whisper transcription on {} samples", audio_data.size());

    int ret = whisper_full(ctx, wparams, audio_data.data(),
                          static_cast<int>(audio_data.size()));

    if (ret != 0) {
        result.error = "Whisper transcription failed with code: " + std::to_string(ret);
        return result;
    }

    // 結果の収集
    int n_segments = whisper_full_n_segments(ctx);
    std::string full_text;

    for (int i = 0; i < n_segments; ++i) {
        const char* segment_text = whisper_full_get_segment_text(ctx, i);
        if (segment_text) {
            full_text += segment_text;
        }
    }

    result.text = full_text;
    result.duration_seconds = static_cast<double>(audio_data.size()) / WHISPER_SAMPLE_RATE;
    result.success = true;

    // 検出された言語を取得（可能な場合）
    if (wparams.detect_language && n_segments > 0) {
        // whisper.cppは言語IDをコンテキストに保存している
        // 単純化のため、ここでは空のままにする
        result.language = "";
    } else if (!params.language.empty()) {
        result.language = params.language;
    }

    spdlog::info("Transcription completed: {} characters, {} segments",
             result.text.size(), n_segments);

    return result;
}

size_t WhisperManager::loadedCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return loaded_models_.size();
}

bool WhisperManager::unloadModel(const std::string& model_path) {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);

    auto it = loaded_models_.find(canonical_path);
    if (it == loaded_models_.end()) {
        return false;
    }

    if (it->second) {
        whisper_free(it->second);
    }
    loaded_models_.erase(it);
    last_access_.erase(canonical_path);

    spdlog::info("Whisper model unloaded: {}", canonical_path);
    return true;
}

std::vector<std::string> WhisperManager::getLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<std::string> models;
    models.reserve(loaded_models_.size());
    for (const auto& [path, _] : loaded_models_) {
        models.push_back(path);
    }
    return models;
}

bool WhisperManager::loadModelIfNeeded(const std::string& model_path) {
    if (isLoaded(model_path)) {
        // アクセス時刻を更新
        std::lock_guard<std::mutex> lock(mutex_);
        updateAccessTime(canonicalizePath(model_path));
        return true;
    }
    return loadModel(model_path);
}

void WhisperManager::setIdleTimeout(std::chrono::milliseconds timeout) {
    std::lock_guard<std::mutex> lock(mutex_);
    idle_timeout_ = timeout;
}

std::chrono::milliseconds WhisperManager::getIdleTimeout() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return idle_timeout_;
}

size_t WhisperManager::unloadIdleModels() {
    std::lock_guard<std::mutex> lock(mutex_);

    auto now = std::chrono::steady_clock::now();
    std::vector<std::string> to_unload;

    for (const auto& [path, last_time] : last_access_) {
        auto idle_duration = std::chrono::duration_cast<std::chrono::milliseconds>(
            now - last_time);
        if (idle_duration >= idle_timeout_) {
            to_unload.push_back(path);
        }
    }

    for (const auto& path : to_unload) {
        auto it = loaded_models_.find(path);
        if (it != loaded_models_.end()) {
            if (it->second) {
                whisper_free(it->second);
            }
            loaded_models_.erase(it);
            last_access_.erase(path);
            spdlog::info("Unloaded idle whisper model: {}", path);
        }
    }

    return to_unload.size();
}

void WhisperManager::setMaxLoadedModels(size_t max_models) {
    std::lock_guard<std::mutex> lock(mutex_);
    max_loaded_models_ = max_models;
}

size_t WhisperManager::getMaxLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return max_loaded_models_;
}

bool WhisperManager::canLoadMore() const {
    // Note: この関数はmutex_がロックされた状態で呼ばれることを想定
    if (max_loaded_models_ == 0) {
        return true;  // 無制限
    }
    return loaded_models_.size() < max_loaded_models_;
}

std::optional<std::chrono::steady_clock::time_point>
WhisperManager::getLastAccessTime(const std::string& model_path) const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);

    auto it = last_access_.find(canonical_path);
    if (it != last_access_.end()) {
        return it->second;
    }
    return std::nullopt;
}

}  // namespace llm_node
