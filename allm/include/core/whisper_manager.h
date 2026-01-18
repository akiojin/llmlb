#pragma once

#include <string>
#include <unordered_map>
#include <memory>
#include <vector>
#include <mutex>
#include <chrono>
#include <optional>

// whisper.cpp forward declarations
struct whisper_context;
struct whisper_full_params;

namespace allm {

/// 音声認識結果
struct TranscriptionResult {
    std::string text;
    std::string language;
    double duration_seconds{0.0};
    bool success{false};
    std::string error;
};

/// Transcription parameters
struct TranscriptionParams {
    std::string language;           // 言語コード ("ja", "en", etc.) 空なら自動検出
    std::string response_format;    // "json", "text", "srt", "vtt"
    bool translate{false};          // 翻訳モード（英語に翻訳）
    int max_threads{4};             // 処理スレッド数
};

/// whisper.cpp によるASR (音声認識) マネージャー
class WhisperManager {
public:
    explicit WhisperManager(std::string models_dir);
    ~WhisperManager();

    // モデルロード
    bool loadModel(const std::string& model_path);

    // WhisperのFlash Attentionを使うか（安定性優先で無効化）
    static bool shouldUseFlashAttention();

    // モデルがロード済みか確認
    bool isLoaded(const std::string& model_path) const;

    // コンテキスト取得
    whisper_context* getContext(const std::string& model_path) const;

    // 音声認識実行
    TranscriptionResult transcribe(
        const std::string& model_path,
        const std::vector<float>& audio_data,
        int sample_rate,
        const TranscriptionParams& params = {});

    // ロード済みモデル数
    size_t loadedCount() const;

    // モデルのアンロード
    bool unloadModel(const std::string& model_path);

    // ロード済みモデルの一覧
    std::vector<std::string> getLoadedModels() const;

    // オンデマンドロード
    bool loadModelIfNeeded(const std::string& model_path);

    // アイドルタイムアウト設定
    void setIdleTimeout(std::chrono::milliseconds timeout);
    std::chrono::milliseconds getIdleTimeout() const;

    // アイドルモデルのアンロード
    size_t unloadIdleModels();

    // 最大ロード数設定
    void setMaxLoadedModels(size_t max_models);
    size_t getMaxLoadedModels() const;

    // ロード可能かチェック
    bool canLoadMore() const;

    // 最終アクセス時刻取得
    std::optional<std::chrono::steady_clock::time_point> getLastAccessTime(
        const std::string& model_path) const;

private:
    std::string models_dir_;
    mutable std::mutex mutex_;
    std::unordered_map<std::string, whisper_context*> loaded_models_;

    // オンデマンドロード用の設定
    std::chrono::milliseconds idle_timeout_{std::chrono::minutes(10)};
    size_t max_loaded_models_{2};  // whisperモデルは大きいので控えめに

    // アクセス時刻追跡
    std::unordered_map<std::string, std::chrono::steady_clock::time_point> last_access_;

    // 正規化されたパスを取得
    std::string canonicalizePath(const std::string& path) const;

    // アクセス時刻を更新
    void updateAccessTime(const std::string& model_path);

    // whisper_full_params を初期化
    whisper_full_params createParams(const TranscriptionParams& params) const;
};

}  // namespace allm
