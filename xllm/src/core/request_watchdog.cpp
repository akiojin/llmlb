#include "core/request_watchdog.h"

#include <atomic>
#include <cerrno>
#include <cstdlib>
#include <optional>

#include <spdlog/spdlog.h>

namespace xllm {

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
    const char* env = std::getenv("XLLM_WATCHDOG_TEST_MODE");
    return env && env[0] != '\0' && env[0] != '0';
}

#ifdef XLLM_TESTING
std::atomic<bool> g_timeout_triggered{false};
#endif
}  // namespace

RequestWatchdog::RequestWatchdog()
    : RequestWatchdog(defaultTimeout()) {}

RequestWatchdog::RequestWatchdog(std::chrono::milliseconds timeout,
                                 std::function<void()> on_timeout)
    : timeout_(timeout)
    , on_timeout_(std::move(on_timeout)) {
    if (timeout_.count() <= 0) {
        timeout_ = defaultTimeout();
    }
    worker_ = std::thread(&RequestWatchdog::run, this);
}

RequestWatchdog::~RequestWatchdog() {
    {
        std::lock_guard<std::mutex> lock(mutex_);
        stop_ = true;
    }
    cv_.notify_one();
    if (worker_.joinable()) {
        worker_.join();
    }
}

std::chrono::milliseconds RequestWatchdog::defaultTimeout() {
    if (auto ms = parse_timeout(std::getenv("XLLM_WATCHDOG_TIMEOUT_MS"), 1)) {
        return *ms;
    }
    if (auto secs = parse_timeout(std::getenv("XLLM_WATCHDOG_TIMEOUT_SECS"), 1000)) {
        return *secs;
    }
    return std::chrono::seconds(30);
}

void RequestWatchdog::run() {
    std::unique_lock<std::mutex> lock(mutex_);
    if (cv_.wait_for(lock, timeout_, [this]() { return stop_; })) {
        return;
    }
    lock.unlock();
    triggerTimeout();
}

void RequestWatchdog::triggerTimeout() {
#ifdef XLLM_TESTING
    if (is_test_mode()) {
        g_timeout_triggered.store(true);
        return;
    }
#endif
    if (on_timeout_) {
        try {
            on_timeout_();
        } catch (...) {
        }
    }
    spdlog::critical("Request watchdog timeout after {}ms. Forcing process exit.", timeout_.count());
    std::_Exit(124);
}

#ifdef XLLM_TESTING
void RequestWatchdog::resetTestState() {
    g_timeout_triggered.store(false);
}

bool RequestWatchdog::wasTimeoutTriggered() {
    return g_timeout_triggered.load();
}
#endif

}  // namespace xllm
