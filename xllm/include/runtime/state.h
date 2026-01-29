#pragma once

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <optional>

namespace xllm {

extern std::atomic<bool> g_running_flag;
extern std::atomic<bool> g_ready_flag;
extern std::atomic<unsigned int> g_active_requests;
extern std::atomic<uint64_t> g_total_requests;
extern std::mutex g_request_mutex;
extern std::condition_variable g_request_cv;

inline bool is_running() { return g_running_flag.load(); }
inline void request_shutdown() { g_running_flag.store(false); }
inline bool is_ready() { return g_ready_flag.load(); }
inline void set_ready(bool v) { g_ready_flag.store(v); }

inline unsigned int active_request_count() { return g_active_requests.load(); }
inline uint64_t total_request_count() { return g_total_requests.load(); }

class RequestGuard {
public:
    RequestGuard() = delete;
    RequestGuard(const RequestGuard&) = delete;
    RequestGuard& operator=(const RequestGuard&) = delete;

    RequestGuard(RequestGuard&& other) noexcept : active_(other.active_) {
        other.active_ = false;
    }

    RequestGuard& operator=(RequestGuard&& other) noexcept {
        if (this != &other) {
            if (active_) {
                g_active_requests.fetch_sub(1);
            }
            active_ = other.active_;
            other.active_ = false;
        }
        return *this;
    }

    ~RequestGuard() {
        if (active_) {
            g_active_requests.fetch_sub(1);
            g_request_cv.notify_one();
        }
    }

    static std::optional<RequestGuard> try_acquire() {
        auto prev = g_active_requests.fetch_add(1);
        if (prev >= 1) {
            g_active_requests.fetch_sub(1);
            return std::nullopt;
        }
        g_total_requests.fetch_add(1);
        return RequestGuard(true);
    }

    static std::optional<RequestGuard> acquire_with_timeout(std::chrono::milliseconds timeout) {
        if (timeout.count() <= 0) {
            return try_acquire();
        }
        std::unique_lock<std::mutex> lock(g_request_mutex);
        auto can_acquire = []() {
            return g_active_requests.load() < 1;
        };
        if (!g_request_cv.wait_for(lock, timeout, can_acquire)) {
            return std::nullopt;
        }
        g_active_requests.fetch_add(1);
        g_total_requests.fetch_add(1);
        return RequestGuard(true);
    }

private:
    explicit RequestGuard(bool active) : active_(active) {}
    bool active_{false};
};

}  // namespace xllm
