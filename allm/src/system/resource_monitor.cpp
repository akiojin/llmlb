#ifndef __APPLE__

#include "system/resource_monitor.h"

#include <spdlog/spdlog.h>
#include <chrono>
#include <exception>
#include <thread>
#include <utility>

#if defined(_WIN32)
#include <windows.h>
#elif defined(__linux__)
#include <sys/sysinfo.h>
#include <unistd.h>
#endif

#ifdef USE_CUDA
#include <nvml.h>
#endif

#ifdef USE_ROCM
#include <rocm_smi/rocm_smi.h>
#endif

namespace allm {
namespace {

struct UsagePair {
    uint64_t used{0};
    uint64_t total{0};
};

UsagePair sample_memory_usage() {
#if defined(_WIN32)
    MEMORYSTATUSEX status;
    status.dwLength = sizeof(status);
    if (!GlobalMemoryStatusEx(&status)) {
        return {};
    }
    UsagePair result;
    result.total = static_cast<uint64_t>(status.ullTotalPhys);
    result.used = result.total - static_cast<uint64_t>(status.ullAvailPhys);
    return result;
#elif defined(__linux__)
    struct sysinfo info;
    if (sysinfo(&info) != 0) {
        return {};
    }
    const uint64_t total = static_cast<uint64_t>(info.totalram) * static_cast<uint64_t>(info.mem_unit);
    const uint64_t free = static_cast<uint64_t>(info.freeram) * static_cast<uint64_t>(info.mem_unit);
    UsagePair result;
    result.total = total;
    result.used = total >= free ? total - free : 0;
    return result;
#else
    return {};
#endif
}

UsagePair sample_vram_usage() {
    UsagePair result;

#ifdef USE_CUDA
    if (nvmlInit() == NVML_SUCCESS) {
        unsigned int device_count = 0;
        if (nvmlDeviceGetCount(&device_count) == NVML_SUCCESS) {
            for (unsigned int i = 0; i < device_count; ++i) {
                nvmlDevice_t device;
                if (nvmlDeviceGetHandleByIndex(i, &device) != NVML_SUCCESS) {
                    continue;
                }
                nvmlMemory_t mem_info;
                if (nvmlDeviceGetMemoryInfo(device, &mem_info) == NVML_SUCCESS) {
                    result.used += static_cast<uint64_t>(mem_info.used);
                    result.total += static_cast<uint64_t>(mem_info.total);
                }
            }
        }
        nvmlShutdown();
    }
#endif

#ifdef USE_ROCM
    if (rsmi_init(0) == RSMI_STATUS_SUCCESS) {
        uint32_t device_count = 0;
        if (rsmi_num_monitor_devices(&device_count) == RSMI_STATUS_SUCCESS) {
            for (uint32_t i = 0; i < device_count; ++i) {
                uint64_t used = 0;
                uint64_t total = 0;
                if (rsmi_dev_memory_usage_get(i, RSMI_MEM_TYPE_VRAM, &used) == RSMI_STATUS_SUCCESS) {
                    result.used += used;
                }
                if (rsmi_dev_memory_total_get(i, RSMI_MEM_TYPE_VRAM, &total) == RSMI_STATUS_SUCCESS) {
                    result.total += total;
                }
            }
        }
        rsmi_shut_down();
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

}  // namespace allm

#endif  // __APPLE__
