#pragma once

#include <httplib.h>
#include <string>
#include <memory>
#include <nlohmann/json.hpp>

namespace allm {

class WhisperManager;
class OnnxTtsManager;

/// OpenAI Audio API互換エンドポイント
/// - POST /v1/audio/transcriptions (ASR via whisper.cpp)
/// - POST /v1/audio/speech (TTS via ONNX Runtime)
class AudioEndpoints {
public:
    /// Constructor for ASR-only mode (whisper.cpp)
    AudioEndpoints(WhisperManager& whisper_manager);

    /// Constructor for ASR + TTS mode (whisper.cpp + ONNX)
    AudioEndpoints(WhisperManager& whisper_manager,
                   OnnxTtsManager& tts_manager);

    void registerRoutes(httplib::Server& server);

private:
    WhisperManager& whisper_manager_;
    OnnxTtsManager* tts_manager_{nullptr};  // Optional TTS support
    // ヘルパーメソッド
    static void setJson(httplib::Response& res, const nlohmann::json& body);
    void respondError(httplib::Response& res, int status,
                      const std::string& code, const std::string& message);

    // ASR エンドポイントハンドラ
    void handleTranscriptions(const httplib::Request& req, httplib::Response& res);

    // TTS エンドポイントハンドラ
    void handleSpeech(const httplib::Request& req, httplib::Response& res);

    // 音声データのデコード（WAV/MP3/FLAC/OGGなどからPCM float配列へ）
    // 返り値: サンプルレート（エラー時は0）
    int decodeAudioToFloat(const std::string& audio_data,
                           const std::string& content_type,
                           std::vector<float>& out_samples);
};

}  // namespace allm
