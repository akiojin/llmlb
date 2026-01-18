#pragma once

#include <chrono>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <thread>

namespace allm {

class RequestWatchdog {
public:
    RequestWatchdog();
    explicit RequestWatchdog(std::chrono::milliseconds timeout,
                             std::function<void()> on_timeout = {});
    ~RequestWatchdog();

    RequestWatchdog(const RequestWatchdog&) = delete;
    RequestWatchdog& operator=(const RequestWatchdog&) = delete;
    RequestWatchdog(RequestWatchdog&&) = delete;
    RequestWatchdog& operator=(RequestWatchdog&&) = delete;

    static std::chrono::milliseconds defaultTimeout();

#ifdef ALLM_TESTING
    static void resetTestState();
    static bool wasTimeoutTriggered();
#endif

private:
    void run();
    void triggerTimeout();

    std::chrono::milliseconds timeout_;
    std::function<void()> on_timeout_;
    std::thread worker_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool stop_{false};
};

}  // namespace allm
