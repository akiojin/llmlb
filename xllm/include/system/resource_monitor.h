#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <functional>
#include <mutex>
#include <thread>

namespace xllm {

inline double usage_ratio(uint64_t used, uint64_t total) {
    if (total == 0) return 0.0;
    return static_cast<double>(used) / static_cast<double>(total);
}

struct ResourceUsage {
    uint64_t mem_used_bytes{0};
    uint64_t mem_total_bytes{0};
    uint64_t vram_used_bytes{0};
    uint64_t vram_total_bytes{0};

    double memUsageRatio() const { return usage_ratio(mem_used_bytes, mem_total_bytes); }
    double vramUsageRatio() const { return usage_ratio(vram_used_bytes, vram_total_bytes); }
};

class ResourceMonitor {
public:
    using MetricsProvider = std::function<ResourceUsage()>;
    using EvictCallback = std::function<bool()>;

    ResourceMonitor(MetricsProvider provider,
                    EvictCallback evict_cb,
                    std::chrono::milliseconds interval = std::chrono::seconds(1),
                    double threshold_ratio = 0.9);

    ResourceMonitor(EvictCallback evict_cb,
                    std::chrono::milliseconds interval = std::chrono::seconds(1),
                    double threshold_ratio = 0.9);

    ~ResourceMonitor();

    void start();
    void stop();
    void pollOnce();
    ResourceUsage latestUsage() const;

    static ResourceUsage sampleSystemUsage();

private:
    bool exceedsThreshold(const ResourceUsage& usage) const;
    void handleThreshold(ResourceUsage& usage);

    MetricsProvider provider_;
    EvictCallback evict_cb_;
    std::chrono::milliseconds interval_;
    double threshold_ratio_;
    std::atomic<bool> running_{false};
    mutable std::mutex mutex_;
    ResourceUsage last_usage_{};
    std::thread worker_;
};

}  // namespace xllm
