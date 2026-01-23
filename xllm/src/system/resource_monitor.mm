#ifdef __APPLE__

#include "system/resource_monitor.h"

#include <spdlog/spdlog.h>
#include <chrono>
#include <exception>
#include <thread>
#include <utility>

#include <mach/mach.h>
#include <sys/sysctl.h>

#ifdef USE_METAL
#import <Metal/Metal.h>
#endif

namespace xllm {
namespace {

struct UsagePair {
    uint64_t used{0};
    uint64_t total{0};
};

UsagePair sample_memory_usage() {
    uint64_t total = 0;
    size_t len = sizeof(total);
    int mib[2] = {CTL_HW, HW_MEMSIZE};
    if (sysctl(mib, 2, &total, &len, nullptr, 0) != 0) {
        total = 0;
    }

    mach_port_t host_port = mach_host_self();
    vm_size_t page_size = 0;
    if (host_page_size(host_port, &page_size) != KERN_SUCCESS) {
        return UsagePair{0, total};
    }

    vm_statistics64_data_t vm_stats;
    mach_msg_type_number_t count = HOST_VM_INFO64_COUNT;
    if (host_statistics64(host_port, HOST_VM_INFO64,
                          reinterpret_cast<host_info64_t>(&vm_stats), &count) != KERN_SUCCESS) {
        return UsagePair{0, total};
    }

    uint64_t used = (static_cast<uint64_t>(vm_stats.active_count) +
                     static_cast<uint64_t>(vm_stats.inactive_count) +
                     static_cast<uint64_t>(vm_stats.wire_count) +
                     static_cast<uint64_t>(vm_stats.compressor_page_count) +
                     static_cast<uint64_t>(vm_stats.speculative_count)) *
        static_cast<uint64_t>(page_size);

    return UsagePair{used, total};
}

UsagePair sample_vram_usage() {
    UsagePair result;

#ifdef USE_METAL
    @autoreleasepool {
        NSArray<id<MTLDevice>>* devices = MTLCopyAllDevices();
        for (id<MTLDevice> device in devices) {
            result.total += static_cast<uint64_t>([device recommendedMaxWorkingSetSize]);
            if ([device respondsToSelector:@selector(currentAllocatedSize)]) {
                result.used += static_cast<uint64_t>([device currentAllocatedSize]);
            }
        }

#if !__has_feature(objc_arc)
        [devices release];
#endif
    }
#endif

    return result;
}

}  // namespace

ResourceMonitor::ResourceMonitor(MetricsProvider provider,
                                 EvictCallback evict_cb,
                                 std::chrono::milliseconds interval,
                                 double threshold_ratio)
    : provider_(std::move(provider)),
      evict_cb_(std::move(evict_cb)),
      interval_(interval),
      threshold_ratio_(threshold_ratio) {}

ResourceMonitor::ResourceMonitor(EvictCallback evict_cb,
                                 std::chrono::milliseconds interval,
                                 double threshold_ratio)
    : ResourceMonitor(sampleSystemUsage, std::move(evict_cb), interval, threshold_ratio) {}

ResourceMonitor::~ResourceMonitor() {
    stop();
}

void ResourceMonitor::start() {
    if (running_.exchange(true)) return;
    worker_ = std::thread([this]() {
        while (running_.load()) {
            pollOnce();
            std::this_thread::sleep_for(interval_);
        }
    });
}

void ResourceMonitor::stop() {
    if (!running_.exchange(false)) return;
    if (worker_.joinable()) {
        worker_.join();
    }
}

void ResourceMonitor::pollOnce() {
    ResourceUsage usage;
    try {
        usage = provider_ ? provider_() : ResourceUsage{};
    } catch (const std::exception& e) {
        spdlog::warn("Resource monitor poll failed: {}", e.what());
        return;
    }

    {
        std::lock_guard<std::mutex> lock(mutex_);
        last_usage_ = usage;
    }

    if (exceedsThreshold(usage)) {
        handleThreshold(usage);
    }
}

ResourceUsage ResourceMonitor::latestUsage() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return last_usage_;
}

ResourceUsage ResourceMonitor::sampleSystemUsage() {
    const auto mem = sample_memory_usage();
    const auto vram = sample_vram_usage();
    return ResourceUsage{mem.used, mem.total, vram.used, vram.total};
}

bool ResourceMonitor::exceedsThreshold(const ResourceUsage& usage) const {
    const bool mem_over = usage.mem_total_bytes > 0 && usage.memUsageRatio() >= threshold_ratio_;
    const bool vram_over = usage.vram_total_bytes > 0 && usage.vramUsageRatio() >= threshold_ratio_;
    return mem_over || vram_over;
}

void ResourceMonitor::handleThreshold(ResourceUsage& usage) {
    if (usage.mem_total_bytes > 0 && usage.memUsageRatio() >= threshold_ratio_) {
        spdlog::warn("Resource monitor: RAM usage {:.1f}% ({} / {} bytes)",
                     usage.memUsageRatio() * 100.0,
                     usage.mem_used_bytes,
                     usage.mem_total_bytes);
    }
    if (usage.vram_total_bytes > 0 && usage.vramUsageRatio() >= threshold_ratio_) {
        spdlog::warn("Resource monitor: VRAM usage {:.1f}% ({} / {} bytes)",
                     usage.vramUsageRatio() * 100.0,
                     usage.vram_used_bytes,
                     usage.vram_total_bytes);
    }

    if (!evict_cb_) return;

    constexpr int kMaxEvictions = 8;
    for (int i = 0; i < kMaxEvictions && exceedsThreshold(usage); ++i) {
        if (!evict_cb_()) {
            break;
        }
        usage = provider_ ? provider_() : ResourceUsage{};
        std::lock_guard<std::mutex> lock(mutex_);
        last_usage_ = usage;
    }
}

}  // namespace xllm

#endif  // __APPLE__
