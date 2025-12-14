#include "api/audio_endpoints.h"
#include "core/whisper_manager.h"
#include "core/onnx_tts_manager.h"

#include <spdlog/spdlog.h>
#include <cstring>
#include <algorithm>

namespace llm_node {

AudioEndpoints::AudioEndpoints(WhisperManager& whisper_manager, const NodeConfig& config)
    : whisper_manager_(whisper_manager), tts_manager_(nullptr), config_(config) {
}

AudioEndpoints::AudioEndpoints(WhisperManager& whisper_manager,
                               OnnxTtsManager& tts_manager,
                               const NodeConfig& config)
    : whisper_manager_(whisper_manager), tts_manager_(&tts_manager), config_(config) {
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
                     "Failed to decode audio file. Supported formats: WAV (16-bit PCM)");
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
    if (!whisper_manager_.loadModelIfNeeded(model_name)) {
        respondError(res, 500, "model_load_failed",
                     "Failed to load model: " + model_name);
        return;
    }

    // Transcription実行
    TranscriptionParams params;
    params.language = language;
    params.response_format = response_format;
    params.max_threads = 4;

    TranscriptionResult result = whisper_manager_.transcribe(
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

void AudioEndpoints::handleSpeech(const httplib::Request& req, httplib::Response& res) {
    spdlog::debug("Handling speech request");

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
    std::string content_type;
    if (params.response_format == "mp3") {
        content_type = "audio/mpeg";
    } else if (params.response_format == "opus") {
        content_type = "audio/opus";
    } else if (params.response_format == "aac") {
        content_type = "audio/aac";
    } else if (params.response_format == "flac") {
        content_type = "audio/flac";
    } else if (params.response_format == "wav") {
        content_type = "audio/wav";
    } else {
        content_type = "audio/pcm";
    }

    // バイナリレスポンス
    res.set_content(
        std::string(reinterpret_cast<const char*>(result.audio_data.data()),
                    result.audio_data.size()),
        content_type);

    spdlog::info("Speech synthesis completed: {} bytes, format={}",
                 result.audio_data.size(), params.response_format);
}

int AudioEndpoints::decodeAudioToFloat(const std::string& audio_data,
                                        const std::string& content_type,
                                        std::vector<float>& out_samples) {
    out_samples.clear();

    // WAV形式のみサポート（他の形式は将来追加）
    if (content_type.find("wav") != std::string::npos ||
        content_type.find("wave") != std::string::npos) {

        int sample_rate, channels, bits_per_sample;
        size_t data_offset, data_size;

        if (!parseWavHeader(audio_data, sample_rate, channels,
                           bits_per_sample, data_offset, data_size)) {
            spdlog::error("Failed to parse WAV header");
            return 0;
        }

        // 16-bit PCMのみサポート
        if (bits_per_sample != 16) {
            spdlog::error("Only 16-bit WAV supported, got {} bits", bits_per_sample);
            return 0;
        }

        // サンプル数を計算
        size_t num_samples = data_size / (bits_per_sample / 8) / channels;
        out_samples.reserve(num_samples);

        const int16_t* samples = reinterpret_cast<const int16_t*>(
            audio_data.data() + data_offset);

        // モノラルに変換しながらfloatに変換
        for (size_t i = 0; i < num_samples; ++i) {
            if (channels == 1) {
                out_samples.push_back(samples[i] / 32768.0f);
            } else {
                // ステレオ→モノラル（平均）
                float sum = 0.0f;
                for (int ch = 0; ch < channels; ++ch) {
                    sum += samples[i * channels + ch] / 32768.0f;
                }
                out_samples.push_back(sum / channels);
            }
        }

        return sample_rate;
    }

    spdlog::error("Unsupported audio format: {}", content_type);
    return 0;
}

bool AudioEndpoints::parseWavHeader(const std::string& data, int& sample_rate,
                                     int& channels, int& bits_per_sample,
                                     size_t& data_offset, size_t& data_size) {
    if (data.size() < 44) {
        return false;
    }

    const uint8_t* buf = reinterpret_cast<const uint8_t*>(data.data());

    // RIFF header
    if (std::memcmp(buf, "RIFF", 4) != 0) {
        return false;
    }

    // WAVE format
    if (std::memcmp(buf + 8, "WAVE", 4) != 0) {
        return false;
    }

    // fmt chunk を探す
    size_t pos = 12;
    while (pos + 8 < data.size()) {
        uint32_t chunk_size;
        std::memcpy(&chunk_size, buf + pos + 4, 4);

        if (std::memcmp(buf + pos, "fmt ", 4) == 0) {
            if (chunk_size < 16 || pos + 8 + chunk_size > data.size()) {
                return false;
            }

            uint16_t audio_format;
            std::memcpy(&audio_format, buf + pos + 8, 2);

            // PCM (1) またはIEEE float (3)のみサポート
            if (audio_format != 1 && audio_format != 3) {
                spdlog::error("Unsupported WAV format: {}", audio_format);
                return false;
            }

            uint16_t num_channels;
            std::memcpy(&num_channels, buf + pos + 10, 2);
            channels = num_channels;

            uint32_t sr;
            std::memcpy(&sr, buf + pos + 12, 4);
            sample_rate = static_cast<int>(sr);

            uint16_t bps;
            std::memcpy(&bps, buf + pos + 22, 2);
            bits_per_sample = bps;

            pos += 8 + chunk_size;
        } else if (std::memcmp(buf + pos, "data", 4) == 0) {
            data_offset = pos + 8;
            data_size = chunk_size;
            return true;
        } else {
            // 他のチャンクをスキップ
            pos += 8 + chunk_size;
        }
    }

    return false;
}

}  // namespace llm_node
