#pragma once

#include <atomic>
#include <optional>

namespace llm_node {

extern std::atomic<bool> g_running_flag;
extern std::atomic<bool> g_ready_flag;
extern std::atomic<unsigned int> g_active_requests;

inline bool is_running() { return g_running_flag.load(); }
inline void request_shutdown() { g_running_flag.store(false); }
inline bool is_ready() { return g_ready_flag.load(); }
inline void set_ready(bool v) { g_ready_flag.store(v); }

inline unsigned int active_request_count() { return g_active_requests.load(); }

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
        }
    }

    static std::optional<RequestGuard> try_acquire() {
        auto prev = g_active_requests.fetch_add(1);
        if (prev >= 1) {
            g_active_requests.fetch_sub(1);
            return std::nullopt;
        }
        return RequestGuard(true);
    }

private:
    explicit RequestGuard(bool active) : active_(active) {}
    bool active_{false};
};

}  // namespace llm_node
