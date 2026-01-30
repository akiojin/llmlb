#pragma once

#include <string>
#include <unordered_map>
#include <memory>
#include <vector>
#include <mutex>
#include <chrono>
#include <optional>
#include <functional>

#ifdef USE_ONNX_RUNTIME
#if __has_include(<onnxruntime/core/session/onnxruntime_cxx_api.h>)
#include <onnxruntime/core/session/onnxruntime_cxx_api.h>
#elif __has_include(<onnxruntime/onnxruntime_cxx_api.h>)
#include <onnxruntime/onnxruntime_cxx_api.h>
#else
#error "onnxruntime_cxx_api.h not found"
#endif
#endif

namespace xllm {

struct SpeechResult {
    std::vector<uint8_t> audio_data;
    std::string format;         // mp3, wav, opus, aac, flac, pcm
    int sample_rate{22050};     // Default for most TTS models
    int channels{1};            // Mono
    int bits_per_sample{16};
    bool success{false};
    std::string error;
};

struct SpeechParams {
    std::string voice{"default"};
    std::string response_format{"mp3"};
    float speed{1.0f};          // 0.25 to 4.0
};

class OnnxTtsManager {
public:
    explicit OnnxTtsManager(std::string models_dir);
    ~OnnxTtsManager();

    // Disable copy
    OnnxTtsManager(const OnnxTtsManager&) = delete;
    OnnxTtsManager& operator=(const OnnxTtsManager&) = delete;

    /// Load a TTS model from path (relative to models_dir or absolute)
    bool loadModel(const std::string& model_path);

    /// Check if model is loaded
    bool isLoaded(const std::string& model_path) const;

    /// Load model if not already loaded (on-demand loading)
    bool loadModelIfNeeded(const std::string& model_path);

    /// Synthesize speech from text
    SpeechResult synthesize(
        const std::string& model_path,
        const std::string& text,
        const SpeechParams& params = {});

    /// Get list of loaded models
    std::vector<std::string> getLoadedModels() const;

    /// Get count of loaded models
    size_t loadedCount() const;

    /// Unload a specific model
    bool unloadModel(const std::string& model_path);

    /// Unload models that have been idle for longer than the timeout
    size_t unloadIdleModels();

    /// Configuration
    void setIdleTimeout(std::chrono::milliseconds timeout);
    std::chrono::milliseconds getIdleTimeout() const;
    void setMaxLoadedModels(size_t max_models);
    size_t getMaxLoadedModels() const;

    /// Get supported voices for a model
    std::vector<std::string> getSupportedVoices(const std::string& model_path) const;

    /// Check if ONNX Runtime is available
    static bool isRuntimeAvailable();

#ifdef XLLM_TESTING
    /// テスト専用: synthesizeの戻り値を差し替える
    void setSynthesizeHookForTest(
        std::function<SpeechResult(const std::string&, const std::string&, const SpeechParams&)> hook);
#endif

private:
    static constexpr const char* kMacosSayModelName = "macos_say";
    static constexpr const char* kVibeVoiceModelId = "microsoft/VibeVoice-Realtime-0.5B";
    static constexpr const char* kVibeVoiceAlias = "vibevoice";

    std::string models_dir_;
    mutable std::mutex mutex_;

#ifdef USE_ONNX_RUNTIME
    Ort::Env env_;
    std::unordered_map<std::string, std::unique_ptr<Ort::Session>> loaded_models_;
#endif

    std::unordered_map<std::string, std::chrono::steady_clock::time_point> last_access_;
    std::chrono::milliseconds idle_timeout_{std::chrono::minutes(30)};
    size_t max_loaded_models_{0};  // 0 = unlimited

#ifdef XLLM_TESTING
    std::function<SpeechResult(const std::string&, const std::string&, const SpeechParams&)> synthesize_hook_;
#endif

    std::string canonicalizePath(const std::string& path) const;
    void updateAccessTime(const std::string& model_path);
    bool canLoadMore() const;

    // Convert raw audio to target format (mp3, wav, etc.)
    std::vector<uint8_t> convertToFormat(
        const std::vector<float>& audio_samples,
        int sample_rate,
        const std::string& format) const;

    // Create WAV header for PCM data
    std::vector<uint8_t> createWavFile(
        const std::vector<float>& samples,
        int sample_rate,
        int channels = 1,
        int bits_per_sample = 16) const;
};

}  // namespace xllm
