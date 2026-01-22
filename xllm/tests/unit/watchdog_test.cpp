#include "core/request_watchdog.h"
#include "core/token_watchdog.h"

#include <chrono>
#include <cstdlib>
#include <optional>
#include <string>
#include <thread>

#include <gtest/gtest.h>

namespace xllm {
namespace {

struct EnvGuard {
    std::string key;
    std::optional<std::string> previous;

    EnvGuard(const std::string& k, const std::string& value)
        : key(k) {
        if (const char* existing = std::getenv(key.c_str())) {
            previous = existing;
        }
#ifdef _WIN32
        _putenv_s(key.c_str(), value.c_str());
#else
        setenv(key.c_str(), value.c_str(), 1);
#endif
    }

    ~EnvGuard() {
#ifdef _WIN32
        if (previous) {
            _putenv_s(key.c_str(), previous->c_str());
        } else {
            _putenv_s(key.c_str(), "");
        }
#else
        if (previous) {
            setenv(key.c_str(), previous->c_str(), 1);
        } else {
            unsetenv(key.c_str());
        }
#endif
    }
};

TEST(RequestWatchdogTest, TimeoutTriggersInTestMode) {
    EnvGuard test_mode("XLLM_WATCHDOG_TEST_MODE", "1");
    EnvGuard timeout_ms("XLLM_WATCHDOG_TIMEOUT_MS", "10");

    RequestWatchdog::resetTestState();
    {
        RequestWatchdog watchdog;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    EXPECT_TRUE(RequestWatchdog::wasTimeoutTriggered());
}

// T182: TokenWatchdog（トークン間タイムアウト）テスト
class TokenWatchdogTest : public ::testing::Test {
protected:
    void SetUp() override {
        TokenWatchdog::resetTestState();
    }
};

TEST_F(TokenWatchdogTest, TimeoutTriggersWhenNoKick) {
    // kickなしで待機すると、タイムアウトがトリガーされる
    EnvGuard test_mode("XLLM_TOKEN_WATCHDOG_TEST_MODE", "1");

    bool timeout_called = false;
    {
        TokenWatchdog watchdog(std::chrono::milliseconds(20), [&timeout_called]() {
            timeout_called = true;
        });
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    EXPECT_TRUE(timeout_called);
    EXPECT_TRUE(TokenWatchdog::wasTimeoutTriggered());
}

TEST_F(TokenWatchdogTest, KickResetsTimeout) {
    // kick keeps the watchdog alive
    EnvGuard test_mode("XLLM_TOKEN_WATCHDOG_TEST_MODE", "1");

    bool timeout_called = false;
    {
#ifdef _WIN32
        const auto timeout = std::chrono::milliseconds(200);
        const auto kick_interval = std::chrono::milliseconds(50);
#else
        const auto timeout = std::chrono::milliseconds(30);
        const auto kick_interval = std::chrono::milliseconds(20);
#endif
        TokenWatchdog watchdog(timeout, [&timeout_called]() {
            timeout_called = true;
        });
        // keep kicking before timeout
        for (int i = 0; i < 5; ++i) {
            std::this_thread::sleep_for(kick_interval);
            watchdog.kick();
        }
    }

    EXPECT_FALSE(timeout_called);
    EXPECT_FALSE(TokenWatchdog::wasTimeoutTriggered());
}

TEST_F(TokenWatchdogTest, TimeoutTriggersAfterKickStops) {
    // kickが止まるとタイムアウトがトリガーされる
    EnvGuard test_mode("XLLM_TOKEN_WATCHDOG_TEST_MODE", "1");

    bool timeout_called = false;
    {
        TokenWatchdog watchdog(std::chrono::milliseconds(20), [&timeout_called]() {
            timeout_called = true;
        });
        // 数回kickした後、停止
        for (int i = 0; i < 3; ++i) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            watchdog.kick();
        }
        // kickなしで待機
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    EXPECT_TRUE(timeout_called);
    EXPECT_TRUE(TokenWatchdog::wasTimeoutTriggered());
}

TEST_F(TokenWatchdogTest, DefaultTimeoutIs5Seconds) {
    // デフォルトタイムアウトは5秒
    EXPECT_EQ(TokenWatchdog::defaultTimeout(), std::chrono::seconds(5));
}

TEST_F(TokenWatchdogTest, TimeoutCanBeConfiguredViaEnv) {
    // 環境変数でタイムアウトを設定可能
    EnvGuard timeout_ms("XLLM_TOKEN_WATCHDOG_TIMEOUT_MS", "100");
    EXPECT_EQ(TokenWatchdog::defaultTimeout(), std::chrono::milliseconds(100));
}

TEST_F(TokenWatchdogTest, StopPreventsTimeout) {
    // stop should prevent timeout
    EnvGuard test_mode("XLLM_TOKEN_WATCHDOG_TEST_MODE", "1");

    bool timeout_called = false;
    {
#ifdef _WIN32
        const auto timeout = std::chrono::milliseconds(200);
        const auto pre_stop_sleep = std::chrono::milliseconds(30);
        const auto post_stop_sleep = std::chrono::milliseconds(150);
#else
        const auto timeout = std::chrono::milliseconds(20);
        const auto pre_stop_sleep = std::chrono::milliseconds(10);
        const auto post_stop_sleep = std::chrono::milliseconds(50);
#endif
        TokenWatchdog watchdog(timeout, [&timeout_called]() {
            timeout_called = true;
        });
        std::this_thread::sleep_for(pre_stop_sleep);
        watchdog.stop();
        std::this_thread::sleep_for(post_stop_sleep);
    }

    EXPECT_FALSE(timeout_called);
    EXPECT_FALSE(TokenWatchdog::wasTimeoutTriggered());
}

}  // namespace
}  // namespace xllm
