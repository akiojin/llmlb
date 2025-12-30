#pragma once

#include <chrono>
#include <cstddef>
#include <string>
#include <utility>
#include <filesystem>
#include <vector>

namespace llm_node {

struct DownloadConfig {
    int max_retries{2};
    std::chrono::milliseconds backoff{200};
    size_t max_concurrency{4};
    size_t max_bytes_per_sec{0};
    size_t chunk_size{4096};
};

DownloadConfig loadDownloadConfig();
std::pair<DownloadConfig, std::string> loadDownloadConfigWithLog();

struct NodeConfig {
    std::string router_url{"http://127.0.0.1:8080"};
    std::string router_api_key;  // API key for router operations (node scope)
    std::string models_dir;
    std::string engine_plugins_dir;
    std::string shared_models_dir;  // Shared router cache mount (optional)
    std::vector<std::string> origin_allowlist;
    int node_port{11435};
    int heartbeat_interval_sec{10};
    bool require_gpu{true};
    std::string bind_address{"0.0.0.0"};
    std::string ip_address;  // Empty means auto-detect
    std::string default_embedding_model{"nomic-embed-text-v1.5"};
};

NodeConfig loadNodeConfig();
std::pair<NodeConfig, std::string> loadNodeConfigWithLog();

}  // namespace llm_node
