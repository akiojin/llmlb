#include "core/request_watchdog.h"

#include <chrono>
#include <cstdlib>
#include <optional>
#include <string>
#include <thread>

#include <gtest/gtest.h>

namespace llm_node {
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
    EnvGuard test_mode("LLM_NODE_WATCHDOG_TEST_MODE", "1");
    EnvGuard timeout_ms("LLM_NODE_WATCHDOG_TIMEOUT_MS", "10");

    RequestWatchdog::resetTestState();
    {
        RequestWatchdog watchdog;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }

    EXPECT_TRUE(RequestWatchdog::wasTimeoutTriggered());
}

}  // namespace
}  // namespace llm_node
