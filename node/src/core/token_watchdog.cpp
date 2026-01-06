#include "core/token_watchdog.h"

#include <atomic>
#include <cerrno>
#include <cstdlib>
#include <optional>

#include <spdlog/spdlog.h>

namespace llm_node {

namespace {
std::optional<long long> parse_positive_integer(const char* value) {
    if (!value || value[0] == '\0') return std::nullopt;
    errno = 0;
    char* end = nullptr;
    long long parsed = std::strtoll(value, &end, 10);
    if (errno != 0 || end == value || (end && *end != '\0') || parsed <= 0) {
        return std::nullopt;
    }
    return parsed;
}

std::optional<std::chrono::milliseconds> parse_timeout(const char* value, long long multiplier) {
    auto parsed = parse_positive_integer(value);
    if (!parsed) return std::nullopt;
    const auto max_ms = std::chrono::milliseconds::max().count();
    if (*parsed > max_ms / multiplier) {
        return std::chrono::milliseconds(max_ms);
    }
    return std::chrono::milliseconds((*parsed) * multiplier);
}

bool is_test_mode() {
    const char* env = std::getenv("LLM_NODE_TOKEN_WATCHDOG_TEST_MODE");
    return env && env[0] != '\0' && env[0] != '0';
}

#ifdef LLM_NODE_TESTING
std::atomic<bool> g_token_timeout_triggered{false};
#endif
}  // namespace

TokenWatchdog::TokenWatchdog()
    : TokenWatchdog(defaultTimeout()) {}

TokenWatchdog::TokenWatchdog(std::chrono::milliseconds timeout,
                             std::function<void()> on_timeout)
    : timeout_(timeout)
    , on_timeout_(std::move(on_timeout)) {
    if (timeout_.count() <= 0) {
        timeout_ = defaultTimeout();
    }
    worker_ = std::thread(&TokenWatchdog::run, this);
}

TokenWatchdog::~TokenWatchdog() {
    stop();
    if (worker_.joinable()) {
        worker_.join();
    }
}

void TokenWatchdog::kick() {
    std::lock_guard<std::mutex> lock(mutex_);
    kicked_ = true;
    cv_.notify_one();
}

void TokenWatchdog::stop() {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        stop_ = true;
    }
    cv_.notify_one();
}

std::chrono::milliseconds TokenWatchdog::defaultTimeout() {
    if (auto ms = parse_timeout(std::getenv("LLM_NODE_TOKEN_WATCHDOG_TIMEOUT_MS"), 1)) {
        return *ms;
    }
    if (auto secs = parse_timeout(std::getenv("LLM_NODE_TOKEN_WATCHDOG_TIMEOUT_SECS"), 1000)) {
        return *secs;
    }
    return std::chrono::seconds(5);
}

void TokenWatchdog::run() {
    std::unique_lock<std::mutex> lock(mutex_);
    while (!stop_) {
        kicked_ = false;
        if (cv_.wait_for(lock, timeout_, [this]() { return stop_ || kicked_; })) {
            if (stop_) {
                return;
            }
            // kicked_ == true の場合、タイマーをリセットして再度待機
            continue;
        }
        // タイムアウト発生
        lock.unlock();
        triggerTimeout();
        return;
    }
}

void TokenWatchdog::triggerTimeout() {
#ifdef LLM_NODE_TESTING
    if (is_test_mode()) {
        g_token_timeout_triggered.store(true);
        if (on_timeout_) {
            try {
                on_timeout_();
            } catch (...) {
            }
        }
        return;
    }
#endif
    if (on_timeout_) {
        try {
            on_timeout_();
        } catch (...) {
        }
    }
    spdlog::error("Token watchdog timeout after {}ms. No token generated.", timeout_.count());
}

#ifdef LLM_NODE_TESTING
void TokenWatchdog::resetTestState() {
    g_token_timeout_triggered.store(false);
}

bool TokenWatchdog::wasTimeoutTriggered() {
    return g_token_timeout_triggered.load();
}
#endif

}  // namespace llm_node
