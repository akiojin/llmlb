#pragma once

#include <httplib.h>
#include <string>
#include <chrono>
#include <vector>
#include "metrics/prometheus_exporter.h"
#include "system/gpu_detector.h"

namespace xllm {

class NodeEndpoints {
public:
    void setGpuInfo(size_t devices, size_t total_mem_bytes, double capability) { gpu_devices_count_ = devices; gpu_total_mem_ = total_mem_bytes; gpu_capability_ = capability; }
    void setGpuDevices(std::vector<GpuDevice> devices);
    NodeEndpoints();
    void registerRoutes(httplib::Server& server);

private:
    std::string health_status_;
    std::chrono::steady_clock::time_point start_time_;
    metrics::PrometheusExporter exporter_;
    size_t gpu_devices_count_{0};
    size_t gpu_total_mem_{0};
    double gpu_capability_{0.0};
    std::vector<GpuDevice> gpu_devices_;
};

}  // namespace xllm
