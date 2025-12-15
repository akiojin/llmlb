#include "core/onnx_tts_manager.h"

#include <spdlog/spdlog.h>
#include <filesystem>
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <stdexcept>

#if defined(__APPLE__) && __has_include(<coreml_provider_factory.h>)
#include <coreml_provider_factory.h>
#endif

namespace llm_node {

OnnxTtsManager::OnnxTtsManager(std::string models_dir)
    : models_dir_(std::move(models_dir))
#ifdef USE_ONNX_RUNTIME
    , env_(ORT_LOGGING_LEVEL_WARNING, "OnnxTtsManager")
#endif
{
    spdlog::info("OnnxTtsManager initialized with models dir: {}", models_dir_);
}

OnnxTtsManager::~OnnxTtsManager() {
#ifdef USE_ONNX_RUNTIME
    std::lock_guard<std::mutex> lock(mutex_);
    loaded_models_.clear();
    spdlog::info("OnnxTtsManager destroyed, all models unloaded");
#endif
}

bool OnnxTtsManager::isRuntimeAvailable() {
#ifdef USE_ONNX_RUNTIME
    return true;
#else
    return false;
#endif
}

std::string OnnxTtsManager::canonicalizePath(const std::string& path) const {
    try {
        if (std::filesystem::path(path).is_absolute()) {
            return std::filesystem::canonical(path).string();
        }
        return std::filesystem::canonical(
            std::filesystem::path(models_dir_) / path).string();
    } catch (const std::filesystem::filesystem_error&) {
        if (std::filesystem::path(path).is_absolute()) {
            return path;
        }
        return (std::filesystem::path(models_dir_) / path).string();
    }
}

void OnnxTtsManager::updateAccessTime(const std::string& model_path) {
    last_access_[model_path] = std::chrono::steady_clock::now();
}

bool OnnxTtsManager::canLoadMore() const {
    if (max_loaded_models_ == 0) {
        return true;  // Unlimited
    }
#ifdef USE_ONNX_RUNTIME
    return loaded_models_.size() < max_loaded_models_;
#else
    return false;
#endif
}

bool OnnxTtsManager::loadModel(const std::string& model_path) {
#ifdef USE_ONNX_RUNTIME
    std::lock_guard<std::mutex> lock(mutex_);

    std::string canonical_path = canonicalizePath(model_path);

    if (loaded_models_.find(canonical_path) != loaded_models_.end()) {
        spdlog::debug("TTS model already loaded: {}", canonical_path);
        updateAccessTime(canonical_path);
        return true;
    }

    if (!canLoadMore()) {
        spdlog::warn("Cannot load more TTS models, max limit reached: {}", max_loaded_models_);
        return false;
    }

    spdlog::info("Loading TTS model: {}", canonical_path);

    try {
        Ort::SessionOptions session_options;
        session_options.SetIntraOpNumThreads(4);
        session_options.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_ALL);
        // CPUフォールバック禁止: EPがサポートできないノードがある場合はセッション生成を失敗させる。
        session_options.AddConfigEntry("session.disable_cpu_ep_fallback", "1");

        // CPUフォールバックは禁止: 非CPUのExecution Providerが必須。
        const auto providers = Ort::GetAvailableProviders();
        auto has_provider = [&](const char* name) {
            return std::find(providers.begin(), providers.end(), name) != providers.end();
        };
        auto has_non_cpu = [&]() {
            for (const auto& p : providers) {
                if (p == "CPUExecutionProvider") continue;
                if (p == "XnnpackExecutionProvider") continue;
                return true;
            }
            return false;
        };
        if (!has_non_cpu()) {
            throw std::runtime_error(
                "ONNX Runtime build has no non-CPU execution providers (CPU-only build).");
        }

        const char* selected = nullptr;
#if defined(__APPLE__)
        if (has_provider("CoreMLExecutionProvider")) {
            selected = "CoreMLExecutionProvider";
        }
#endif
        if (selected == nullptr && has_provider("CUDAExecutionProvider")) {
            selected = "CUDAExecutionProvider";
        }
        if (selected == nullptr && has_provider("TensorrtExecutionProvider")) {
            selected = "TensorrtExecutionProvider";
        }
        if (selected == nullptr && has_provider("TensorRTExecutionProvider")) {
            selected = "TensorRTExecutionProvider";
        }
        if (selected == nullptr && has_provider("ROCMExecutionProvider")) {
            selected = "ROCMExecutionProvider";
        }
        if (selected == nullptr && has_provider("DirectMLExecutionProvider")) {
            selected = "DirectMLExecutionProvider";
        }
        if (selected == nullptr && has_provider("OpenVINOExecutionProvider")) {
            selected = "OpenVINOExecutionProvider";
        }
        if (selected == nullptr) {
            throw std::runtime_error(
                "No supported hardware execution provider found (expected CoreML/CUDA/ROCm/etc).");
        }
        if (std::strcmp(selected, "CoreMLExecutionProvider") == 0) {
#if defined(__APPLE__) && __has_include(<coreml_provider_factory.h>)
            const uint32_t coreml_flags = COREML_FLAG_ENABLE_ON_SUBGRAPH;
            Ort::ThrowOnError(OrtSessionOptionsAppendExecutionProvider_CoreML(session_options, coreml_flags));
            spdlog::info("ONNX Runtime: CoreMLExecutionProvider enabled (TTS)");
#else
            throw std::runtime_error(
                "CoreMLExecutionProvider is required but coreml_provider_factory.h is not available.");
#endif
        } else {
            session_options.AppendExecutionProvider(selected);
            spdlog::info("ONNX Runtime: {} enabled (TTS)", selected);
        }

        auto session = std::make_unique<Ort::Session>(
            env_, canonical_path.c_str(), session_options);

        loaded_models_[canonical_path] = std::move(session);
        updateAccessTime(canonical_path);

        spdlog::info("TTS model loaded successfully: {}", canonical_path);
        return true;
    } catch (const std::exception& e) {
        spdlog::error("Failed to load TTS model: {} - {}", canonical_path, e.what());
        return false;
    }
#else
    spdlog::warn("ONNX Runtime not available, cannot load TTS model: {}", model_path);
    return false;
#endif
}

bool OnnxTtsManager::isLoaded(const std::string& model_path) const {
#ifdef USE_ONNX_RUNTIME
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);
    return loaded_models_.find(canonical_path) != loaded_models_.end();
#else
    (void)model_path;
    return false;
#endif
}

bool OnnxTtsManager::loadModelIfNeeded(const std::string& model_path) {
    if (isLoaded(model_path)) {
        std::lock_guard<std::mutex> lock(mutex_);
        updateAccessTime(canonicalizePath(model_path));
        return true;
    }
    return loadModel(model_path);
}

SpeechResult OnnxTtsManager::synthesize(
    const std::string& model_path,
    const std::string& text,
    const SpeechParams& params) {

    SpeechResult result;

#ifdef USE_ONNX_RUNTIME
    if (text.empty()) {
        result.error = "Empty text input";
        return result;
    }

    if (params.response_format != "wav") {
        result.error =
            "Only 'wav' response_format is supported for now (requested: " + params.response_format +
            ")";
        return result;
    }

    std::string canonical_path = canonicalizePath(model_path);

    Ort::Session* session = nullptr;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        auto it = loaded_models_.find(canonical_path);
        if (it == loaded_models_.end()) {
            result.error = "Model not loaded: " + canonical_path;
            return result;
        }
        session = it->second.get();
        updateAccessTime(canonical_path);
    }

    spdlog::debug("Running TTS synthesis on {} characters", text.size());

    try {
        Ort::AllocatorWithDefaultOptions allocator;
        Ort::MemoryInfo memory_info = Ort::MemoryInfo::CreateCpu(
            OrtArenaAllocator, OrtMemTypeDefault);

        // Get input/output names
        size_t num_inputs = session->GetInputCount();
        size_t num_outputs = session->GetOutputCount();

        if (num_inputs < 1 || num_outputs < 1) {
            result.error = "Invalid TTS model: expected at least 1 input and 1 output tensor";
            return result;
        }

        std::vector<const char*> input_names;
        std::vector<const char*> output_names;
        std::vector<Ort::AllocatedStringPtr> input_name_ptrs;
        std::vector<Ort::AllocatedStringPtr> output_name_ptrs;

        for (size_t i = 0; i < num_inputs; ++i) {
            auto name = session->GetInputNameAllocated(i, allocator);
            input_names.push_back(name.get());
            input_name_ptrs.push_back(std::move(name));
        }
        for (size_t i = 0; i < num_outputs; ++i) {
            auto name = session->GetOutputNameAllocated(i, allocator);
            output_names.push_back(name.get());
            output_name_ptrs.push_back(std::move(name));
        }

        // Minimal implementation for PoC:
        // - Require first input to be float tensor
        // - Encode raw UTF-8 bytes of `text` into a fixed-size float feature vector
        // - Require first output to be float/float16 tensor representing waveform samples
        {
            Ort::TypeInfo input_type_info = session->GetInputTypeInfo(0);
            auto input_tensor_info = input_type_info.GetTensorTypeAndShapeInfo();
            const auto input_elem_type = input_tensor_info.GetElementType();
            if (input_elem_type != ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) {
                result.error = "Unsupported TTS input tensor type (expected float)";
                return result;
            }

            auto input_shape = input_tensor_info.GetShape();
            if (input_shape.empty()) {
                result.error = "Unsupported TTS input shape (empty)";
                return result;
            }

            // Normalize to 2-D shape [1, N] for the PoC.
            int64_t feature_len = -1;
            if (input_shape.size() == 1) {
                feature_len = input_shape[0];
                input_shape = {1, input_shape[0]};
            } else if (input_shape.size() == 2) {
                feature_len = input_shape[1];
                input_shape[0] = 1;
            } else {
                result.error = "Unsupported TTS input rank (expected 1 or 2)";
                return result;
            }

            if (feature_len <= 0) {
                feature_len = 32;
                input_shape[1] = feature_len;
            }

            // NOTE: This is a PoC-only "frontend". The toy model maps features[0] directly
            // to an audible tone waveform so users can verify audio output.
            std::vector<float> features(static_cast<size_t>(feature_len), 0.0f);
            features[0] = 1.0f;

            auto input_tensor = Ort::Value::CreateTensor<float>(
                memory_info,
                features.data(),
                features.size(),
                input_shape.data(),
                input_shape.size());

            auto outputs = session->Run(
                Ort::RunOptions{nullptr},
                &input_names[0],
                &input_tensor,
                1,
                &output_names[0],
                1);

            if (outputs.empty() || !outputs[0].IsTensor()) {
                result.error = "TTS inference returned no tensor output";
                return result;
            }

            auto out_info = outputs[0].GetTensorTypeAndShapeInfo();
            const auto out_elem_type = out_info.GetElementType();
            auto out_shape = out_info.GetShape();

            if (out_shape.empty()) {
                result.error = "TTS output has empty shape";
                return result;
            }

            // Flatten output to 1-D samples.
            size_t num_samples = 1;
            for (const auto d : out_shape) {
                if (d <= 0) {
                    result.error = "TTS output has dynamic/unknown shape";
                    return result;
                }
                num_samples *= static_cast<size_t>(d);
            }

            std::vector<float> samples;
            samples.reserve(num_samples);

            if (out_elem_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) {
                const float* data = outputs[0].GetTensorData<float>();
                samples.assign(data, data + num_samples);
            } else if (out_elem_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT16) {
                const uint16_t* data = outputs[0].GetTensorData<uint16_t>();
                // IEEE 754 half -> float conversion (minimal, sufficient for PoC).
                auto half_to_float = [](uint16_t h) -> float {
                    const uint16_t sign = (h >> 15) & 0x1;
                    const uint16_t exp = (h >> 10) & 0x1F;
                    const uint16_t frac = h & 0x3FF;

                    uint32_t f_sign = static_cast<uint32_t>(sign) << 31;
                    uint32_t f_exp;
                    uint32_t f_frac;

                    if (exp == 0) {
                        if (frac == 0) {
                            f_exp = 0;
                            f_frac = 0;
                        } else {
                            // Subnormal
                            int shift = 0;
                            uint16_t mant = frac;
                            while ((mant & 0x400) == 0) {
                                mant <<= 1;
                                ++shift;
                            }
                            mant &= 0x3FF;
                            f_exp = static_cast<uint32_t>(127 - 15 - shift) << 23;
                            f_frac = static_cast<uint32_t>(mant) << 13;
                        }
                    } else if (exp == 31) {
                        // Inf/NaN
                        f_exp = 0xFFu << 23;
                        f_frac = static_cast<uint32_t>(frac) << 13;
                    } else {
                        f_exp = static_cast<uint32_t>(exp + (127 - 15)) << 23;
                        f_frac = static_cast<uint32_t>(frac) << 13;
                    }

                    uint32_t f_bits = f_sign | f_exp | f_frac;
                    float out;
                    std::memcpy(&out, &f_bits, sizeof(out));
                    return out;
                };

                for (size_t i = 0; i < num_samples; ++i) {
                    samples.push_back(half_to_float(data[i]));
                }
            } else {
                result.error = "Unsupported TTS output tensor type (expected float/float16)";
                return result;
            }

            // Encode to WAV (16-bit PCM).
            constexpr int kSampleRate = 16000;
            result.audio_data = createWavFile(samples, kSampleRate, /*channels=*/1, /*bits_per_sample=*/16);
            result.format = "wav";
            result.sample_rate = kSampleRate;
            result.channels = 1;
            result.bits_per_sample = 16;
            result.success = true;
            return result;
        }

    } catch (const Ort::Exception& e) {
        result.error = std::string("ONNX inference failed: ") + e.what();
        return result;
    }
#else
    (void)model_path;
    (void)text;
    (void)params;
    result.error = "ONNX Runtime not available. Build with -DBUILD_WITH_ONNX=ON";
    return result;
#endif
}

std::vector<std::string> OnnxTtsManager::getLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<std::string> models;
#ifdef USE_ONNX_RUNTIME
    models.reserve(loaded_models_.size());
    for (const auto& [path, _] : loaded_models_) {
        models.push_back(path);
    }
#endif
    return models;
}

size_t OnnxTtsManager::loadedCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
#ifdef USE_ONNX_RUNTIME
    return loaded_models_.size();
#else
    return 0;
#endif
}

bool OnnxTtsManager::unloadModel(const std::string& model_path) {
#ifdef USE_ONNX_RUNTIME
    std::lock_guard<std::mutex> lock(mutex_);
    std::string canonical_path = canonicalizePath(model_path);

    auto it = loaded_models_.find(canonical_path);
    if (it == loaded_models_.end()) {
        return false;
    }

    loaded_models_.erase(it);
    last_access_.erase(canonical_path);

    spdlog::info("TTS model unloaded: {}", canonical_path);
    return true;
#else
    (void)model_path;
    return false;
#endif
}

size_t OnnxTtsManager::unloadIdleModels() {
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

#ifdef USE_ONNX_RUNTIME
    for (const auto& path : to_unload) {
        auto it = loaded_models_.find(path);
        if (it != loaded_models_.end()) {
            loaded_models_.erase(it);
            last_access_.erase(path);
            spdlog::info("Unloaded idle TTS model: {}", path);
        }
    }
#endif

    return to_unload.size();
}

void OnnxTtsManager::setIdleTimeout(std::chrono::milliseconds timeout) {
    std::lock_guard<std::mutex> lock(mutex_);
    idle_timeout_ = timeout;
}

std::chrono::milliseconds OnnxTtsManager::getIdleTimeout() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return idle_timeout_;
}

void OnnxTtsManager::setMaxLoadedModels(size_t max_models) {
    std::lock_guard<std::mutex> lock(mutex_);
    max_loaded_models_ = max_models;
}

size_t OnnxTtsManager::getMaxLoadedModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return max_loaded_models_;
}

std::vector<std::string> OnnxTtsManager::getSupportedVoices(const std::string& model_path) const {
    // For now, return a default voice list
    // Real implementation would query the model for supported voices
    (void)model_path;
    return {"alloy", "echo", "fable", "onyx", "nova", "shimmer"};
}

std::vector<uint8_t> OnnxTtsManager::convertToFormat(
    const std::vector<float>& audio_samples,
    int sample_rate,
    const std::string& format) const {

    if (format == "wav" || format == "pcm") {
        return createWavFile(audio_samples, sample_rate);
    }

    // For other formats (mp3, opus, aac, flac), we would need additional libraries
    // For now, fall back to WAV
    spdlog::warn("Format '{}' not yet supported, falling back to WAV", format);
    return createWavFile(audio_samples, sample_rate);
}

std::vector<uint8_t> OnnxTtsManager::createWavFile(
    const std::vector<float>& samples,
    int sample_rate,
    int channels,
    int bits_per_sample) const {

    std::vector<uint8_t> wav;

    // Calculate sizes
    size_t data_size = samples.size() * (bits_per_sample / 8);
    size_t file_size = 44 + data_size;  // Header + data

    wav.reserve(file_size);

    // RIFF header
    wav.push_back('R'); wav.push_back('I'); wav.push_back('F'); wav.push_back('F');

    // File size - 8
    uint32_t chunk_size = static_cast<uint32_t>(file_size - 8);
    wav.push_back(chunk_size & 0xFF);
    wav.push_back((chunk_size >> 8) & 0xFF);
    wav.push_back((chunk_size >> 16) & 0xFF);
    wav.push_back((chunk_size >> 24) & 0xFF);

    // WAVE format
    wav.push_back('W'); wav.push_back('A'); wav.push_back('V'); wav.push_back('E');

    // fmt subchunk
    wav.push_back('f'); wav.push_back('m'); wav.push_back('t'); wav.push_back(' ');

    // Subchunk1 size (16 for PCM)
    wav.push_back(16); wav.push_back(0); wav.push_back(0); wav.push_back(0);

    // Audio format (1 = PCM)
    wav.push_back(1); wav.push_back(0);

    // Number of channels
    wav.push_back(channels & 0xFF); wav.push_back((channels >> 8) & 0xFF);

    // Sample rate
    wav.push_back(sample_rate & 0xFF);
    wav.push_back((sample_rate >> 8) & 0xFF);
    wav.push_back((sample_rate >> 16) & 0xFF);
    wav.push_back((sample_rate >> 24) & 0xFF);

    // Byte rate
    uint32_t byte_rate = sample_rate * channels * (bits_per_sample / 8);
    wav.push_back(byte_rate & 0xFF);
    wav.push_back((byte_rate >> 8) & 0xFF);
    wav.push_back((byte_rate >> 16) & 0xFF);
    wav.push_back((byte_rate >> 24) & 0xFF);

    // Block align
    uint16_t block_align = channels * (bits_per_sample / 8);
    wav.push_back(block_align & 0xFF);
    wav.push_back((block_align >> 8) & 0xFF);

    // Bits per sample
    wav.push_back(bits_per_sample & 0xFF);
    wav.push_back((bits_per_sample >> 8) & 0xFF);

    // data subchunk
    wav.push_back('d'); wav.push_back('a'); wav.push_back('t'); wav.push_back('a');

    // Data size
    uint32_t data_size_32 = static_cast<uint32_t>(data_size);
    wav.push_back(data_size_32 & 0xFF);
    wav.push_back((data_size_32 >> 8) & 0xFF);
    wav.push_back((data_size_32 >> 16) & 0xFF);
    wav.push_back((data_size_32 >> 24) & 0xFF);

    // Convert float samples to 16-bit PCM
    for (float sample : samples) {
        // Clamp to [-1, 1]
        sample = std::max(-1.0f, std::min(1.0f, sample));
        // Convert to int16
        int16_t pcm_sample = static_cast<int16_t>(sample * 32767.0f);
        wav.push_back(pcm_sample & 0xFF);
        wav.push_back((pcm_sample >> 8) & 0xFF);
    }

    return wav;
}

}  // namespace llm_node
