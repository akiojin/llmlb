/**
 * @file plugin_logger_test.cpp
 * @brief T183/T190: プラグインログ統合テスト
 *
 * PluginLoggerクラスのユニットテスト。
 * プラグインのstdout/stderrをホストログに統合する機能をテスト。
 */

#include <gtest/gtest.h>
#include "core/plugin_logger.h"
#include <spdlog/spdlog.h>
#include <spdlog/sinks/ostream_sink.h>
#include <sstream>
#include <thread>
#include <chrono>

using namespace llm_node;

class PluginLoggerTest : public ::testing::Test {
protected:
    void SetUp() override {
        // テスト用のストリームシンクを設定
        log_stream_.str("");
        auto sink = std::make_shared<spdlog::sinks::ostream_sink_mt>(log_stream_);
        test_logger_ = std::make_shared<spdlog::logger>("test_plugin_logger", sink);
        test_logger_->set_pattern("[%Y-%m-%d %H:%M:%S.%e] [%l] %v");
        test_logger_->set_level(spdlog::level::trace);
    }

    std::ostringstream log_stream_;
    std::shared_ptr<spdlog::logger> test_logger_;
};

// T190-1: プラグインIDプレフィックス付与
TEST_F(PluginLoggerTest, AddsPluginIdPrefix) {
    PluginLogger logger("gptoss", test_logger_);

    logger.log(LogLevel::kInfo, "Test message");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("[gptoss]") != std::string::npos)
        << "Output: " << output;
    EXPECT_TRUE(output.find("Test message") != std::string::npos);
}

// T190-2: ログレベル付与
TEST_F(PluginLoggerTest, AddsLogLevel) {
    PluginLogger logger("llama_cpp", test_logger_);

    logger.log(LogLevel::kWarning, "Warning message");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("[warning]") != std::string::npos)
        << "Output: " << output;
}

// T190-3: 各ログレベルの出力
TEST_F(PluginLoggerTest, SupportsAllLogLevels) {
    PluginLogger logger("test", test_logger_);

    logger.log(LogLevel::kTrace, "trace");
    logger.log(LogLevel::kDebug, "debug");
    logger.log(LogLevel::kInfo, "info");
    logger.log(LogLevel::kWarning, "warning");
    logger.log(LogLevel::kError, "error");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("trace") != std::string::npos);
    EXPECT_TRUE(output.find("debug") != std::string::npos);
    EXPECT_TRUE(output.find("info") != std::string::npos);
    EXPECT_TRUE(output.find("warning") != std::string::npos);
    EXPECT_TRUE(output.find("error") != std::string::npos);
}

// T190-4: タイムスタンプ付与
TEST_F(PluginLoggerTest, AddsTimestamp) {
    PluginLogger logger("nemotron", test_logger_);

    logger.log(LogLevel::kInfo, "Timestamped");
    test_logger_->flush();

    std::string output = log_stream_.str();
    // パターン: [YYYY-MM-DD HH:MM:SS.mmm]
    EXPECT_TRUE(output.find("[202") != std::string::npos)
        << "Output should contain timestamp: " << output;
}

// T190-5: 複数行メッセージの処理
TEST_F(PluginLoggerTest, HandlesMultilineMessages) {
    PluginLogger logger("test", test_logger_);

    logger.log(LogLevel::kInfo, "Line1\nLine2\nLine3");
    test_logger_->flush();

    std::string output = log_stream_.str();
    // 各行にプレフィックスが付くことを確認
    size_t count = 0;
    size_t pos = 0;
    while ((pos = output.find("[test]", pos)) != std::string::npos) {
        count++;
        pos += 6;
    }
    EXPECT_EQ(count, 3) << "Each line should have prefix: " << output;
}

// T190-6: 空メッセージの処理
TEST_F(PluginLoggerTest, HandlesEmptyMessage) {
    PluginLogger logger("test", test_logger_);

    logger.log(LogLevel::kInfo, "");
    test_logger_->flush();

    std::string output = log_stream_.str();
    // 空メッセージは出力されない
    EXPECT_TRUE(output.empty() || output.find("[test]") == std::string::npos);
}

// T190-7: 特殊文字を含むメッセージ
TEST_F(PluginLoggerTest, HandlesSpecialCharacters) {
    PluginLogger logger("test", test_logger_);

    logger.log(LogLevel::kInfo, "Special: {json} %s \\n \t");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("{json}") != std::string::npos);
}

// T190-8: 長いプラグインID
TEST_F(PluginLoggerTest, HandlesLongPluginId) {
    PluginLogger logger("very_long_plugin_engine_name_for_testing", test_logger_);

    logger.log(LogLevel::kInfo, "Message");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("[very_long_plugin_engine_name_for_testing]") != std::string::npos);
}

// T190-9: スレッドセーフティ
TEST_F(PluginLoggerTest, ThreadSafety) {
    PluginLogger logger("threaded", test_logger_);

    std::vector<std::thread> threads;
    for (int i = 0; i < 10; i++) {
        threads.emplace_back([&logger, i]() {
            for (int j = 0; j < 100; j++) {
                logger.log(LogLevel::kInfo, "Thread " + std::to_string(i) + " msg " + std::to_string(j));
            }
        });
    }

    for (auto& t : threads) {
        t.join();
    }

    test_logger_->flush();
    std::string output = log_stream_.str();

    // 1000件のログが出力されていることを確認
    size_t count = 0;
    size_t pos = 0;
    while ((pos = output.find("[threaded]", pos)) != std::string::npos) {
        count++;
        pos += 10;
    }
    EXPECT_EQ(count, 1000);
}

// T190-10: デフォルトロガー使用
TEST_F(PluginLoggerTest, UsesDefaultLoggerWhenNotSpecified) {
    PluginLogger logger("default_test");

    // デフォルトロガーでも例外なく動作
    EXPECT_NO_THROW(logger.log(LogLevel::kInfo, "Default logger test"));
}

// T190-11: プラグインIDの取得
TEST_F(PluginLoggerTest, GetPluginId) {
    PluginLogger logger("my_plugin", test_logger_);

    EXPECT_EQ(logger.pluginId(), "my_plugin");
}

// T190-12: ログレベルフィルタリング
TEST_F(PluginLoggerTest, RespectsLogLevelFilter) {
    // ロガーをwarning以上に設定
    test_logger_->set_level(spdlog::level::warn);
    PluginLogger logger("filtered", test_logger_);

    logger.log(LogLevel::kDebug, "debug");
    logger.log(LogLevel::kInfo, "info");
    logger.log(LogLevel::kWarning, "warning");
    logger.log(LogLevel::kError, "error");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("debug") == std::string::npos);
    EXPECT_TRUE(output.find("info") == std::string::npos);
    EXPECT_TRUE(output.find("warning") != std::string::npos);
    EXPECT_TRUE(output.find("error") != std::string::npos);
}

// T190-13: ストリームキャプチャ開始/停止
TEST_F(PluginLoggerTest, StreamCaptureStartStop) {
    PluginLogger logger("capture", test_logger_);

    // キャプチャ開始
    EXPECT_TRUE(logger.startCapture());
    EXPECT_TRUE(logger.isCapturing());

    // キャプチャ停止
    logger.stopCapture();
    EXPECT_FALSE(logger.isCapturing());
}

// T190-14: 便利メソッド
TEST_F(PluginLoggerTest, ConvenienceMethods) {
    PluginLogger logger("convenience", test_logger_);

    logger.info("info message");
    logger.warn("warn message");
    logger.error("error message");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("info message") != std::string::npos);
    EXPECT_TRUE(output.find("warn message") != std::string::npos);
    EXPECT_TRUE(output.find("error message") != std::string::npos);
}

// T190-15: フォーマット文字列サポート
TEST_F(PluginLoggerTest, FormatStringSupport) {
    PluginLogger logger("format", test_logger_);

    logger.info("Value: {}, Name: {}", 42, "test");
    test_logger_->flush();

    std::string output = log_stream_.str();
    EXPECT_TRUE(output.find("Value: 42") != std::string::npos);
    EXPECT_TRUE(output.find("Name: test") != std::string::npos);
}
