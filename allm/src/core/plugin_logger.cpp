/**
 * @file plugin_logger.cpp
 * @brief T183/T190: プラグインログのホスト統合
 *
 * プラグインのstdout/stderrをホストログシステムに統合。
 */

#include "core/plugin_logger.h"
#include <sstream>

namespace allm {

PluginLogger::PluginLogger(const std::string& plugin_id,
                           std::shared_ptr<spdlog::logger> logger)
    : plugin_id_(plugin_id),
      logger_(logger ? logger : spdlog::default_logger()) {
}

PluginLogger::~PluginLogger() {
    if (capturing_) {
        stopCapture();
    }
}

PluginLogger::PluginLogger(PluginLogger&& other) noexcept
    : plugin_id_(std::move(other.plugin_id_)),
      logger_(std::move(other.logger_)),
      capturing_(other.capturing_) {
    other.capturing_ = false;
}

PluginLogger& PluginLogger::operator=(PluginLogger&& other) noexcept {
    if (this != &other) {
        if (capturing_) {
            stopCapture();
        }
        plugin_id_ = std::move(other.plugin_id_);
        logger_ = std::move(other.logger_);
        capturing_ = other.capturing_;
        other.capturing_ = false;
    }
    return *this;
}

void PluginLogger::log(LogLevel level, const std::string& message) {
    if (message.empty()) {
        return;
    }

    // 複数行メッセージを処理
    if (message.find('\n') != std::string::npos) {
        logLines(level, message);
        return;
    }

    // プラグインIDプレフィックスを付けてログ出力
    std::string formatted = fmt::format("[{}] {}", plugin_id_, message);
    logger_->log(toSpdlogLevel(level), formatted);
}

void PluginLogger::info(const std::string& message) {
    log(LogLevel::kInfo, message);
}

void PluginLogger::warn(const std::string& message) {
    log(LogLevel::kWarning, message);
}

void PluginLogger::error(const std::string& message) {
    log(LogLevel::kError, message);
}

bool PluginLogger::startCapture() {
    if (capturing_) {
        return false;
    }
    capturing_ = true;
    return true;
}

void PluginLogger::stopCapture() {
    capturing_ = false;
}

void PluginLogger::logLines(LogLevel level, const std::string& message) {
    std::istringstream stream(message);
    std::string line;
    while (std::getline(stream, line)) {
        if (!line.empty()) {
            std::string formatted = fmt::format("[{}] {}", plugin_id_, line);
            logger_->log(toSpdlogLevel(level), formatted);
        }
    }
}

spdlog::level::level_enum PluginLogger::toSpdlogLevel(LogLevel level) {
    switch (level) {
        case LogLevel::kTrace:
            return spdlog::level::trace;
        case LogLevel::kDebug:
            return spdlog::level::debug;
        case LogLevel::kInfo:
            return spdlog::level::info;
        case LogLevel::kWarning:
            return spdlog::level::warn;
        case LogLevel::kError:
            return spdlog::level::err;
        default:
            return spdlog::level::info;
    }
}

std::string PluginLogger::levelToString(LogLevel level) {
    switch (level) {
        case LogLevel::kTrace:
            return "trace";
        case LogLevel::kDebug:
            return "debug";
        case LogLevel::kInfo:
            return "info";
        case LogLevel::kWarning:
            return "warning";
        case LogLevel::kError:
            return "error";
        default:
            return "info";
    }
}

}  // namespace allm
