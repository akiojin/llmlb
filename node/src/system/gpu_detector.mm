// This file is only compiled on macOS (Objective-C++)
#ifdef __APPLE__

#include "system/gpu_detector.h"
#include <iostream>

#ifdef USE_METAL
#import <Metal/Metal.h>
#endif

namespace llm_node {

GpuDetector::GpuDetector() {
    // Constructor
}

GpuDetector::~GpuDetector() {
    // Destructor
}

std::vector<GpuDevice> GpuDetector::detect() {
    detected_devices_.clear();

#ifdef USE_METAL
    auto metal_devices = detectMetal();
    detected_devices_.insert(detected_devices_.end(), metal_devices.begin(), metal_devices.end());
#endif

    // Fallback if Metal support not compiled
    if (detected_devices_.empty()) {
        GpuDevice dev;
        dev.id = 0;
        dev.name = "Apple GPU (Metal support not compiled)";
        dev.memory_bytes = 0;  // Unknown
        dev.free_memory_bytes = 0;
        dev.compute_capability = "unknown";
        dev.vendor = "apple";
        dev.is_available = false;  // Not usable without Metal
        detected_devices_.push_back(dev);
    }

    return detected_devices_;
}

bool GpuDetector::hasGpu() const {
    // Check if any GPU is available and usable
    for (const auto& dev : detected_devices_) {
        if (dev.is_available) {
            return true;
        }
    }
    return false;
}

bool GpuDetector::requireGpu() const {
    return hasGpu();
}

std::unique_ptr<GpuDevice> GpuDetector::getGpuById(int id) const {
    for (const auto& dev : detected_devices_) {
        if (dev.id == id) {
            return std::make_unique<GpuDevice>(dev);
        }
    }
    return nullptr;
}

size_t GpuDetector::getTotalMemory() const {
    size_t total = 0;
    for (const auto& dev : detected_devices_) {
        if (dev.is_available) {
            total += dev.memory_bytes;
        }
    }
    return total;
}

double GpuDetector::getCapabilityScore() const {
    // Calculate a capability score based on memory and compute capability
    // This is used by the router for load balancing
    double score = 0.0;

    for (const auto& dev : detected_devices_) {
        if (!dev.is_available) continue;

        // Base score from memory (GB)
        double mem_score = static_cast<double>(dev.memory_bytes) / (1024.0 * 1024.0 * 1024.0);

        // Apple Silicon GPUs are generally efficient
        double cc_factor = 1.5;

        score += mem_score * cc_factor;
    }

    return score;
}

GpuBackend GpuDetector::getGpuBackend() const {
    for (const auto& dev : detected_devices_) {
        if (!dev.is_available) {
            continue;
        }
        if (dev.vendor == "apple") {
            return GpuBackend::Metal;
        }
        if (dev.vendor == "nvidia") {
            return GpuBackend::Cuda;
        }
        if (dev.vendor == "amd") {
            return GpuBackend::Rocm;
        }
        if (dev.vendor == "directml") {
            return GpuBackend::DirectML;
        }
    }
    return GpuBackend::Cpu;
}

std::optional<int> GpuDetector::selectGpu(std::optional<int> prefer_loaded_gpu) const {
    const GpuDevice* preferred = nullptr;
    if (prefer_loaded_gpu.has_value()) {
        for (const auto& dev : detected_devices_) {
            if (dev.id == prefer_loaded_gpu.value() && dev.is_available) {
                preferred = &dev;
                break;
            }
        }
    }
    if (preferred) return preferred->id;

    const GpuDevice* best = nullptr;
    size_t best_free = 0;
    for (const auto& dev : detected_devices_) {
        if (!dev.is_available) continue;
        const size_t free_bytes = dev.free_memory_bytes > 0 ? dev.free_memory_bytes : dev.memory_bytes;
        if (!best || free_bytes > best_free) {
            best = &dev;
            best_free = free_bytes;
        }
    }
    if (best) return best->id;
    return std::nullopt;
}

GpuBackend GpuDetector::getGpuBackend() const {
    // Find the first available GPU and return its backend
    for (const auto& dev : detected_devices_) {
        if (!dev.is_available) continue;

        if (dev.vendor == "nvidia") {
            return GpuBackend::kCuda;
        } else if (dev.vendor == "apple") {
            return GpuBackend::kMetal;
        } else if (dev.vendor == "amd") {
            return GpuBackend::kRocm;
        }
    }
    return GpuBackend::kCpu;
}

std::vector<GpuDevice> GpuDetector::detectCuda() {
    // CUDA is not available on macOS
    return std::vector<GpuDevice>();
}

std::vector<GpuDevice> GpuDetector::detectMetal() {
    std::vector<GpuDevice> devices;

#ifdef USE_METAL
    // Metal detection for macOS
    @autoreleasepool {
        NSArray<id<MTLDevice>>* metal_devices = MTLCopyAllDevices();

        int device_id = 0;
        for (id<MTLDevice> mtl_device in metal_devices) {
            GpuDevice dev;
            dev.id = device_id++;
            dev.name = [[mtl_device name] UTF8String];

            // Get recommended working set size (approximate available memory)
            dev.memory_bytes = [mtl_device recommendedMaxWorkingSetSize];
            dev.free_memory_bytes = dev.memory_bytes;
            if ([mtl_device respondsToSelector:@selector(currentAllocatedSize)]) {
                const uint64_t used = static_cast<uint64_t>([mtl_device currentAllocatedSize]);
                if (dev.memory_bytes >= used) {
                    dev.free_memory_bytes = dev.memory_bytes - used;
                }
            }

            // Metal doesn't have compute capability like CUDA
            if ([mtl_device supportsFamily:MTLGPUFamilyMetal3]) {
                dev.compute_capability = "Metal3";
            } else if ([mtl_device supportsFamily:MTLGPUFamilyApple7]) {
                dev.compute_capability = "Apple7";
            } else {
                dev.compute_capability = "Metal";
            }

            dev.vendor = "apple";
            dev.is_available = true;

            devices.push_back(dev);
        }

#if !__has_feature(objc_arc)
        [metal_devices release];
#endif
    }
#endif

    return devices;
}

std::vector<GpuDevice> GpuDetector::detectRocm() {
    // ROCm is not available on macOS
    return std::vector<GpuDevice>();
}

#ifdef LLM_NODE_TESTING
void GpuDetector::setDetectedDevicesForTest(std::vector<GpuDevice> devices) {
    detected_devices_ = std::move(devices);
}
#endif

} // namespace llm_node

#endif // __APPLE__
