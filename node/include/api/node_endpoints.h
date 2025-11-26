#pragma once

#include <httplib.h>
#include <string>
#include <atomic>
#include <chrono>
#include "metrics/prometheus_exporter.h"

namespace ollama_node {

class NodeEndpoints {
public:
    void setGpuInfo(size_t devices, size_t total_mem_bytes, double capability) { gpu_devices_ = devices; gpu_total_mem_ = total_mem_bytes; gpu_capability_ = capability; }
    NodeEndpoints();
    void registerRoutes(httplib::Server& server);

private:
    std::string health_status_;
    std::chrono::steady_clock::time_point start_time_;
    std::atomic<uint64_t> pull_count_{0};
    metrics::PrometheusExporter exporter_;
    size_t gpu_devices_{0};
    size_t gpu_total_mem_{0};
    double gpu_capability_{0.0};
};

}  // namespace ollama_node
