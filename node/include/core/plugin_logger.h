/**
 * @file plugin_logger.h
 * @brief T183/T190: プラグインログのホスト統合
 *
 * プラグインのstdout/stderrをホストログシステムに統合。
 * プラグインIDプレフィックス、タイムスタンプ、ログレベルを付与。
 */

#pragma once

#include <functional>
#include <memory>
#include <string>
#include <spdlog/spdlog.h>

namespace llm_node {

/**
 * @brief ログレベル
 */
enum class LogLevel {
    kTrace,
    kDebug,
    kInfo,
    kWarning,
    kError
};

/**
 * @class PluginLogger
 * @brief プラグインログをホストログシステムに統合するクラス
 *
 * 使用例:
 * @code
 * PluginLogger logger("gptoss");
 * logger.info("Model loaded: {}", model_name);
 * logger.warn("Low VRAM: {} MB remaining", vram_mb);
 * @endcode
 */
class PluginLogger {
public:
    /**
     * @brief コンストラクタ
     * @param plugin_id プラグイン識別子
     * @param logger 使用するspdlogロガー（省略時はデフォルト）
     */
    explicit PluginLogger(const std::string& plugin_id,
                          std::shared_ptr<spdlog::logger> logger = nullptr);

    ~PluginLogger();

    // コピー禁止
    PluginLogger(const PluginLogger&) = delete;
    PluginLogger& operator=(const PluginLogger&) = delete;

    // ムーブ許可
    PluginLogger(PluginLogger&& other) noexcept;
    PluginLogger& operator=(PluginLogger&& other) noexcept;

    /**
     * @brief ログを出力
     * @param level ログレベル
     * @param message メッセージ
     */
    void log(LogLevel level, const std::string& message);

    /**
     * @brief INFOレベルでログ出力
     */
    void info(const std::string& message);

    /**
     * @brief INFOレベルでフォーマット付きログ出力
     */
    template<typename... Args>
    void info(fmt::format_string<Args...> format, Args&&... args) {
        log(LogLevel::kInfo, fmt::format(format, std::forward<Args>(args)...));
    }

    /**
     * @brief WARNINGレベルでログ出力
     */
    void warn(const std::string& message);

    /**
     * @brief WARNINGレベルでフォーマット付きログ出力
     */
    template<typename... Args>
    void warn(fmt::format_string<Args...> format, Args&&... args) {
        log(LogLevel::kWarning, fmt::format(format, std::forward<Args>(args)...));
    }

    /**
     * @brief ERRORレベルでログ出力
     */
    void error(const std::string& message);

    /**
     * @brief ERRORレベルでフォーマット付きログ出力
     */
    template<typename... Args>
    void error(fmt::format_string<Args...> format, Args&&... args) {
        log(LogLevel::kError, fmt::format(format, std::forward<Args>(args)...));
    }

    /**
     * @brief プラグインIDを取得
     */
    const std::string& pluginId() const { return plugin_id_; }

    /**
     * @brief stdout/stderrキャプチャを開始
     * @return 成功した場合true
     */
    bool startCapture();

    /**
     * @brief stdout/stderrキャプチャを停止
     */
    void stopCapture();

    /**
     * @brief キャプチャ中かどうか
     */
    bool isCapturing() const { return capturing_; }

private:
    std::string plugin_id_;
    std::shared_ptr<spdlog::logger> logger_;
    bool capturing_{false};

    /**
     * @brief 複数行メッセージを分割して出力
     */
    void logLines(LogLevel level, const std::string& message);

    /**
     * @brief LogLevelをspdlog::levelに変換
     */
    static spdlog::level::level_enum toSpdlogLevel(LogLevel level);

    /**
     * @brief LogLevelを文字列に変換
     */
    static std::string levelToString(LogLevel level);
};

}  // namespace llm_node
