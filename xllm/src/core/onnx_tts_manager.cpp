#include "core/onnx_tts_manager.h"

#include <spdlog/spdlog.h>
#include <filesystem>
#include <algorithm>
#include <cstdint>
#include <cstring>
#include <cstdlib>
#include <fstream>
#include <stdexcept>

#if defined(__APPLE__)
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>
extern char** environ;
#endif

namespace xllm {

namespace {

#if defined(__APPLE__)
int run_command(const std::vector<std::string>& args) {
    if (args.empty()) {
        return -1;
    }

    std::vector<char*> argv;
    argv.reserve(args.size() + 1);
    for (const auto& a : args) {
        argv.push_back(const_cast<char*>(a.c_str()));
    }
    argv.push_back(nullptr);

    pid_t pid;
    int spawn_result = posix_spawnp(&pid, argv[0], nullptr, nullptr, argv.data(), environ);
    if (spawn_result != 0) {
        return spawn_result;
    }

    int status = 0;
    if (waitpid(pid, &status, 0) == -1) {
        return -1;
    }
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status)) {
        return 128 + WTERMSIG(status);
    }
    return -1;
}

std::vector<uint8_t> read_file_bytes(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        throw std::runtime_error("Failed to open file: " + path.string());
    }

    in.seekg(0, std::ios::end);
    std::streamsize size = in.tellg();
    if (size < 0) {
        throw std::runtime_error("Failed to stat file size: " + path.string());
    }
    in.seekg(0, std::ios::beg);

    std::vector<uint8_t> data(static_cast<size_t>(size));
    if (!in.read(reinterpret_cast<char*>(data.data()), size)) {
        throw std::runtime_error("Failed to read file: " + path.string());
    }
    return data;
}
#endif

bool isVibeVoice(const std::string& model_path) {
    // Check for various forms of VibeVoice model specification
    return model_path == "vibevoice" ||
           model_path == "microsoft/VibeVoice-Realtime-0.5B" ||
           model_path.find("vibevoice") != std::string::npos ||
           model_path.find("VibeVoice") != std::string::npos;
}

std::string mapVibeVoicePrompt(const std::string& voice, const std::string& default_voice) {
    static const std::unordered_map<std::string, std::string> kOpenAiToVibeVoice = {
        {"alloy", "Carter"},
        {"echo", "Carter"},
        {"fable", "Daphne"},
        {"onyx", "Carter"},
        {"nova", "Nicole"},
        {"shimmer", "Ruby"},
    };
    static const std::vector<std::string> kVibeVoicePrompts = {
        "Carter",
        "Nicole",
        "Aria",
        "Daphne",
        "Jessica",
        "Ruby",
    };

    auto it = kOpenAiToVibeVoice.find(voice);
    if (it != kOpenAiToVibeVoice.end()) {
        return it->second;
    }

    if (std::find(kVibeVoicePrompts.begin(), kVibeVoicePrompts.end(), voice) != kVibeVoicePrompts.end()) {
        return voice;
    }

    return default_voice;
}

}  // namespace

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
    // VibeVoice is handled by external Python runner, no ONNX loading needed
    if (isVibeVoice(model_path)) {
        spdlog::info("VibeVoice model registered (uses external Python runner): {}", model_path);
        return true;
    }

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

        auto session = std::make_unique<Ort::Session>(
            env_, canonical_path.c_str(), session_options);

        loaded_models_[canonical_path] = std::move(session);
        updateAccessTime(canonical_path);

        spdlog::info("TTS model loaded successfully: {}", canonical_path);
        return true;
    } catch (const Ort::Exception& e) {
        spdlog::error("Failed to load TTS model: {} - {}", canonical_path, e.what());
        return false;
    }
#else
    spdlog::warn("ONNX Runtime not available, cannot load TTS model: {}", model_path);
    return false;
#endif
}

bool OnnxTtsManager::isLoaded(const std::string& model_path) const {
    // VibeVoice is always "loaded" since it uses external Python runner
    if (isVibeVoice(model_path)) {
        return true;
    }

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

    if (text.empty()) {
        result.error = "Empty text input";
        return result;
    }

    // Handle VibeVoice via external Python runner
    if (isVibeVoice(model_path)) {
#if defined(__APPLE__)
        // Get configuration from environment variables
        const char* runner_env = std::getenv("XLLM_VIBEVOICE_RUNNER");
        if (!runner_env || std::strlen(runner_env) == 0) {
            result.error = "XLLM_VIBEVOICE_RUNNER environment variable not set";
            return result;
        }
        std::string runner_path = runner_env;

        const char* python_env = std::getenv("XLLM_VIBEVOICE_PYTHON");
        std::string python_bin = python_env ? python_env : "python3";

        const char* device_env = std::getenv("XLLM_VIBEVOICE_DEVICE");
        std::string device = device_env ? device_env : "mps";

        const char* model_env = std::getenv("XLLM_VIBEVOICE_MODEL");
        std::string model_id = model_env ? model_env : "microsoft/VibeVoice-Realtime-0.5B";

        const char* ddpm_env = std::getenv("XLLM_VIBEVOICE_DDPM_STEPS");
        std::string ddpm_steps = ddpm_env ? ddpm_env : "5";

        const char* cfg_env = std::getenv("XLLM_VIBEVOICE_CFG_SCALE");
        std::string cfg_scale = cfg_env ? cfg_env : "1.5";

        const char* voice_env = std::getenv("XLLM_VIBEVOICE_DEFAULT_VOICE");
        std::string default_voice = voice_env ? voice_env : "Carter";

        // Use voice from params if specified, otherwise use default
        std::string requested_voice = (params.voice.empty() || params.voice == "default")
                                      ? default_voice : params.voice;
        std::string voice = mapVibeVoicePrompt(requested_voice, default_voice);
        if (voice != requested_voice) {
            spdlog::info("Mapped voice '{}' to VibeVoice prompt '{}'", requested_voice, voice);
        }

        const char* voice_prompt_env = std::getenv("XLLM_VIBEVOICE_VOICE_PROMPT");
        std::string voice_prompt_path = voice_prompt_env ? voice_prompt_env : "";

        // Create temporary directory for output
        auto temp_dir = std::filesystem::temp_directory_path() /
                        ("vibevoice_" + std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
        std::filesystem::create_directories(temp_dir);
        auto output_path = temp_dir / "output.wav";

        spdlog::info("Running VibeVoice synthesis: voice={}, device={}, text_len={}",
                     voice, device, text.size());

        // Build command arguments
        std::vector<std::string> args = {
            python_bin,
            runner_path,
            "--model", model_id,
            "--device", device,
            "--ddpm-steps", ddpm_steps,
            "--cfg-scale", cfg_scale,
            "--voice", voice,
            "--text", text,
            "--out", output_path.string()
        };

        if (!voice_prompt_path.empty()) {
            args.emplace_back("--voice-prompt");
            args.emplace_back(voice_prompt_path);
        }

        // Execute the Python runner
        int exit_code = run_command(args);
        if (exit_code != 0) {
            std::filesystem::remove_all(temp_dir);
            result.error = "VibeVoice runner failed with exit code: " + std::to_string(exit_code);
            return result;
        }

        // Read the output WAV file
        try {
            result.audio_data = read_file_bytes(output_path);
            result.format = "wav";
            result.sample_rate = 22050;  // VibeVoice default
            result.channels = 1;
            result.bits_per_sample = 16;
            result.success = true;
        } catch (const std::exception& e) {
            result.error = std::string("Failed to read VibeVoice output: ") + e.what();
        }

        // Cleanup
        std::filesystem::remove_all(temp_dir);
        return result;
#else
        result.error = "VibeVoice is only supported on macOS";
        return result;
#endif
    }

#ifdef USE_ONNX_RUNTIME
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
        // Note: Actual ONNX model inference depends on the specific model architecture
        // This is a placeholder showing the general pattern
        // Real implementation would need model-specific input/output handling

        Ort::AllocatorWithDefaultOptions allocator;
        Ort::MemoryInfo memory_info = Ort::MemoryInfo::CreateCpu(
            OrtArenaAllocator, OrtMemTypeDefault);

        // Get input/output names
        size_t num_inputs = session->GetInputCount();
        size_t num_outputs = session->GetOutputCount();

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

        // This is a simplified placeholder - actual implementation
        // depends on the specific TTS model's input format
        // Most TTS models expect tokenized/encoded text input

        // For now, return an error indicating model-specific implementation needed
        result.error = "TTS model inference not yet implemented for this model type";
        return result;

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

}  // namespace xllm
