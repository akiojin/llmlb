#pragma once

#include <httplib.h>
#include <string>
#include <atomic>
#include <chrono>
#include <memory>
#include "metrics/prometheus_exporter.h"

namespace llm_node {

// Forward declarations
class ModelSync;
class RouterClient;

class NodeEndpoints {
public:
    void setGpuInfo(size_t devices, size_t total_mem_bytes, double capability) { gpu_devices_ = devices; gpu_total_mem_ = total_mem_bytes; gpu_capability_ = capability; }
    NodeEndpoints();
    void registerRoutes(httplib::Server& server);

    // Dependency injection for model sync and router client
    void setModelSync(std::shared_ptr<ModelSync> sync);
    void setRouterClient(std::shared_ptr<RouterClient> client);

private:
    std::string health_status_;
    std::chrono::steady_clock::time_point start_time_;
    metrics::PrometheusExporter exporter_;
    size_t gpu_devices_{0};
    size_t gpu_total_mem_{0};
    double gpu_capability_{0.0};

    // Request counters
    std::atomic<uint64_t> request_count_{0};
    std::atomic<uint64_t> pull_count_{0};

    // Dependencies for model pull
    std::shared_ptr<ModelSync> model_sync_;
    std::shared_ptr<RouterClient> router_client_;
};

}  // namespace llm_node
