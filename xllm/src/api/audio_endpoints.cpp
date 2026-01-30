#include "api/audio_endpoints.h"
#include "core/audio_manager.h"
#include "core/onnx_tts_manager.h"
#include "runtime/state.h"

#include <spdlog/spdlog.h>
#include <cstring>
#include <algorithm>
#include <cstddef>
#include <chrono>
#include <cstdlib>

#define MINIAUDIO_IMPLEMENTATION
#define MA_NO_DEVICE_IO
#define MA_NO_ENGINE
#define MA_NO_NODE_GRAPH
#define MA_NO_RESOURCE_MANAGER
#define MA_NO_THREADING
#include "vendor/miniaudio/miniaudio.h"

namespace xllm {

namespace {
std::chrono::milliseconds get_request_queue_timeout() {
    constexpr int64_t kDefaultTimeoutMs = 1000;
    if (const char* env = std::getenv("XLLM_REQUEST_QUEUE_TIMEOUT_MS")) {
        try {
            int64_t value = std::stoll(env);
            if (value > 0) {
                return std::chrono::milliseconds(value);
            }
        } catch (...) {
        }
    }
    return std::chrono::milliseconds(kDefaultTimeoutMs);
}
}  // namespace

AudioEndpoints::AudioEndpoints(AudioManager& audio_manager)
    : audio_manager_(audio_manager), tts_manager_(nullptr) {
}

AudioEndpoints::AudioEndpoints(AudioManager& audio_manager,
                               OnnxTtsManager& tts_manager)
    : audio_manager_(audio_manager), tts_manager_(&tts_manager) {
}

void AudioEndpoints::setJson(httplib::Response& res, const nlohmann::json& body) {
    res.set_content(body.dump(), "application/json");
}

void AudioEndpoints::respondError(httplib::Response& res, int status,
                                   const std::string& code, const std::string& message) {
    res.status = status;
    setJson(res, {
        {"error", {
            {"message", message},
            {"type", "invalid_request_error"},
            {"code", code}
        }}
    });
}

void AudioEndpoints::registerRoutes(httplib::Server& server) {
    // ASR endpoint (whisper.cpp)
    server.Post("/v1/audio/transcriptions",
        [this](const httplib::Request& req, httplib::Response& res) {
            handleTranscriptions(req, res);
        });

    // TTS endpoint (ONNX Runtime)
    server.Post("/v1/audio/speech",
        [this](const httplib::Request& req, httplib::Response& res) {
            handleSpeech(req, res);
        });

    std::string endpoints = "/v1/audio/transcriptions";
    if (tts_manager_) {
        endpoints += ", /v1/audio/speech";
    }
    spdlog::info("Audio endpoints registered: {}", endpoints);
}

void AudioEndpoints::handleTranscriptions(const httplib::Request& req, httplib::Response& res) {
    spdlog::debug("Handling transcription request");

    auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
    if (!guard) {
        respondError(res, 429, "too_many_requests", "Node is busy");
        return;
    }

    // multipart/form-dataの検証
    if (!req.form.has_file("file")) {
        respondError(res, 400, "missing_file", "Missing required field: file");
        return;
    }

    // ファイルデータの取得
    const auto file = req.form.get_file("file");
    if (file.content.empty()) {
        respondError(res, 400, "empty_file", "Audio file is empty");
        return;
    }

    // モデル名の取得
    std::string model_name;
    if (req.form.has_field("model")) {
        model_name = req.form.get_field("model");
    } else {
        respondError(res, 400, "missing_model", "Missing required field: model");
        return;
    }

    // オプションパラメータ
    std::string language;
    if (req.form.has_field("language")) {
        language = req.form.get_field("language");
    }

    std::string response_format = "json";
    if (req.form.has_field("response_format")) {
        response_format = req.form.get_field("response_format");
    }

    // Content-Typeの推測
    std::string content_type = file.content_type;
    if (content_type.empty()) {
        // ファイル名から推測
        std::string filename = file.filename;
        std::transform(filename.begin(), filename.end(), filename.begin(), ::tolower);
        if (filename.ends_with(".wav")) {
            content_type = "audio/wav";
        } else if (filename.ends_with(".mp3")) {
            content_type = "audio/mpeg";
        } else if (filename.ends_with(".flac")) {
            content_type = "audio/flac";
        } else {
            content_type = "audio/wav";  // デフォルト
        }
    }

    // 音声データをfloat配列にデコード
    std::vector<float> audio_samples;
    int sample_rate = decodeAudioToFloat(file.content, content_type, audio_samples);

    if (sample_rate == 0) {
        respondError(res, 400, "invalid_audio",
                     "Failed to decode audio file. Supported formats: WAV, MP3, FLAC, OGG");
        return;
    }

    // サンプルレートのリサンプリング（必要な場合）
    // whisper.cppは16kHzを期待
    if (sample_rate != 16000) {
        // 簡易的な線形リサンプリング
        std::vector<float> resampled;
        double ratio = 16000.0 / sample_rate;
        size_t new_size = static_cast<size_t>(audio_samples.size() * ratio);
        resampled.reserve(new_size);

        for (size_t i = 0; i < new_size; ++i) {
            double src_pos = i / ratio;
            size_t idx = static_cast<size_t>(src_pos);
            double frac = src_pos - idx;

            if (idx + 1 < audio_samples.size()) {
                resampled.push_back(static_cast<float>(
                    audio_samples[idx] * (1.0 - frac) + audio_samples[idx + 1] * frac));
            } else if (idx < audio_samples.size()) {
                resampled.push_back(audio_samples[idx]);
            }
        }
        audio_samples = std::move(resampled);
        sample_rate = 16000;
    }

    // モデルのオンデマンドロード
    if (!audio_manager_.loadModelIfNeeded(model_name)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model_name);
        return;
    }

    // Transcription実行
    TranscriptionParams params;
    params.language = language;
    params.response_format = response_format;
    params.max_threads = 4;

    TranscriptionResult result = audio_manager_.transcribe(
        model_name, audio_samples, sample_rate, params);

    if (!result.success) {
        respondError(res, 500, "transcription_failed", result.error);
        return;
    }

    // レスポンス形式に応じた出力
    if (response_format == "text") {
        res.set_content(result.text, "text/plain");
    } else if (response_format == "srt") {
        // SRT形式（簡易版）
        std::string srt = "1\n00:00:00,000 --> " +
            std::to_string(static_cast<int>(result.duration_seconds / 60)) + ":" +
            std::to_string(static_cast<int>(result.duration_seconds) % 60) + ":000\n" +
            result.text + "\n";
        res.set_content(srt, "text/plain");
    } else if (response_format == "vtt") {
        // VTT形式（簡易版）
        std::string vtt = "WEBVTT\n\n00:00:00.000 --> " +
            std::to_string(static_cast<int>(result.duration_seconds / 60)) + ":" +
            std::to_string(static_cast<int>(result.duration_seconds) % 60) + ".000\n" +
            result.text + "\n";
        res.set_content(vtt, "text/vtt");
    } else {
        // JSON形式（デフォルト）
        nlohmann::json response = {
            {"text", result.text}
        };
        if (!result.language.empty()) {
            response["language"] = result.language;
        }
        setJson(res, response);
    }

    spdlog::info("Transcription completed: {} chars", result.text.size());
}

namespace {

// T304: 内部バッファ満杯相当（時間ベース）でチャンクサイズを決定する
size_t computeStreamingChunkBytes(const SpeechResult& result) {
    // フォーマット情報が不十分な場合は固定サイズにフォールバック
    if (result.sample_rate <= 0 || result.channels <= 0 || result.bits_per_sample <= 0) {
        return 16 * 1024;
    }

    const size_t bytes_per_sample = static_cast<size_t>(result.bits_per_sample / 8);
    if (bytes_per_sample == 0) {
        return 16 * 1024;
    }

    const size_t bytes_per_second =
        static_cast<size_t>(result.sample_rate) *
        static_cast<size_t>(result.channels) *
        bytes_per_sample;

    // 200ms相当のバッファで送る（小さすぎ/大きすぎを避ける）
    const size_t by_duration = bytes_per_second / 5;
    constexpr size_t kMinChunk = 4 * 1024;
    constexpr size_t kMaxChunk = 64 * 1024;
    return std::clamp(by_duration, kMinChunk, kMaxChunk);
}

}  // namespace

void AudioEndpoints::handleSpeech(const httplib::Request& req, httplib::Response& res) {
    spdlog::debug("Handling speech request");

    auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
    if (!guard) {
        respondError(res, 429, "too_many_requests", "Node is busy");
        return;
    }

    // TTS manager が未設定の場合
    if (!tts_manager_) {
        respondError(res, 501, "not_implemented",
                     "TTS support not available. Build with -DBUILD_WITH_ONNX=ON");
        return;
    }

    // JSON bodyのパース
    nlohmann::json body;
    try {
        body = nlohmann::json::parse(req.body);
    } catch (const nlohmann::json::parse_error& e) {
        respondError(res, 400, "invalid_json",
                     std::string("Invalid JSON: ") + e.what());
        return;
    }

    // 必須パラメータ: model
    if (!body.contains("model") || !body["model"].is_string()) {
        respondError(res, 400, "missing_model", "Missing required field: model");
        return;
    }
    std::string model_name = body["model"].get<std::string>();

    // 必須パラメータ: input (text to speak)
    if (!body.contains("input") || !body["input"].is_string()) {
        respondError(res, 400, "missing_input", "Missing required field: input");
        return;
    }
    std::string input_text = body["input"].get<std::string>();

    if (input_text.empty()) {
        respondError(res, 400, "empty_input", "Input text is empty");
        return;
    }

    // オプションパラメータ
    SpeechParams params;

    if (body.contains("voice") && body["voice"].is_string()) {
        params.voice = body["voice"].get<std::string>();
    }

    if (body.contains("response_format") && body["response_format"].is_string()) {
        params.response_format = body["response_format"].get<std::string>();
        // Validate format
        static const std::vector<std::string> valid_formats = {
            "mp3", "opus", "aac", "flac", "wav", "pcm"
        };
        if (std::find(valid_formats.begin(), valid_formats.end(),
                      params.response_format) == valid_formats.end()) {
            respondError(res, 400, "invalid_format",
                         "Invalid response_format. Valid formats: mp3, opus, aac, flac, wav, pcm");
            return;
        }
    }

    if (body.contains("speed") && body["speed"].is_number()) {
        params.speed = body["speed"].get<float>();
        if (params.speed < 0.25f || params.speed > 4.0f) {
            respondError(res, 400, "invalid_speed",
                         "Speed must be between 0.25 and 4.0");
            return;
        }
    }

    const bool stream = body.value("stream", false);

    // モデルのオンデマンドロード
    if (!tts_manager_->loadModelIfNeeded(model_name)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load TTS model: " + model_name);
        return;
    }

    // 音声合成実行
    SpeechResult result = tts_manager_->synthesize(model_name, input_text, params);

    if (!result.success) {
        respondError(res, 500, "synthesis_failed", result.error);
        return;
    }

    // Content-Typeの設定
    std::string output_format = params.response_format;
    if (!result.format.empty()) {
        if (result.format != params.response_format) {
            spdlog::info("Overriding response_format from '{}' to '{}' based on synthesis output",
                         params.response_format, result.format);
        }
        output_format = result.format;
    }

    std::string content_type;
    if (output_format == "mp3") {
        content_type = "audio/mpeg";
    } else if (output_format == "opus") {
        content_type = "audio/opus";
    } else if (output_format == "aac") {
        content_type = "audio/aac";
    } else if (output_format == "flac") {
        content_type = "audio/flac";
    } else if (output_format == "wav") {
        content_type = "audio/wav";
    } else {
        content_type = "audio/pcm";
    }

    const size_t audio_bytes = result.audio_data.size();

    if (stream) {
        // T301/T305: チャンク単位でのストリーミング送信（chunked transfer）
        res.set_header("Content-Type", content_type);

        const size_t chunk_bytes = computeStreamingChunkBytes(result);
        auto audio = std::make_shared<std::vector<uint8_t>>(std::move(result.audio_data));
        auto guard_ptr = std::make_shared<decltype(guard)>(std::move(guard));

        res.set_chunked_content_provider(
            content_type,
            [audio, guard_ptr, chunk_bytes](size_t offset, httplib::DataSink& sink) {
                (void)guard_ptr;  // keep guard alive for the duration of the stream

                if (offset >= audio->size()) {
                    sink.done();
                    return true;
                }

                const size_t remaining = audio->size() - offset;
                const size_t to_write = std::min(chunk_bytes, remaining);
                sink.write(reinterpret_cast<const char*>(audio->data() + offset), to_write);

                if (offset + to_write >= audio->size()) {
                    sink.done();
                }
                return true;
            });
    } else {
        // バイナリレスポンス（非ストリーミング）
        res.set_content(
            std::string(reinterpret_cast<const char*>(result.audio_data.data()),
                        result.audio_data.size()),
            content_type);
    }

    spdlog::info("Speech synthesis completed: {} bytes, format={}",
                 audio_bytes, output_format);
}

int AudioEndpoints::decodeAudioToFloat(const std::string& audio_data,
                                        const std::string& content_type,
                                        std::vector<float>& out_samples) {
    out_samples.clear();

    (void)content_type;

    ma_decoder_config config = ma_decoder_config_init(ma_format_f32, 0, 0);
    ma_uint64 frame_count = 0;
    void* pcm_frames = nullptr;

    ma_result result = ma_decode_memory(audio_data.data(), audio_data.size(),
                                        &config, &frame_count, &pcm_frames);

    if (result != MA_SUCCESS || pcm_frames == nullptr || frame_count == 0) {
        spdlog::error("Failed to decode audio data (miniaudio result={})",
                      static_cast<int>(result));
        return 0;
    }

    const ma_uint32 channels = config.channels;
    const ma_uint32 sample_rate = config.sampleRate;
    const float* samples = static_cast<const float*>(pcm_frames);
    const size_t total_samples = static_cast<size_t>(frame_count) * channels;

    if (channels == 1) {
        out_samples.assign(samples, samples + total_samples);
    } else {
        out_samples.reserve(static_cast<size_t>(frame_count));
        for (ma_uint64 i = 0; i < frame_count; ++i) {
            float sum = 0.0f;
            for (ma_uint32 ch = 0; ch < channels; ++ch) {
                sum += samples[i * channels + ch];
            }
            out_samples.push_back(sum / static_cast<float>(channels));
        }
    }

    ma_free(pcm_frames, nullptr);
    return static_cast<int>(sample_rate);
}

}  // namespace xllm
