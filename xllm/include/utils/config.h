#pragma once

#include <chrono>
#include <cstddef>
#include <string>
#include <utility>
#include <filesystem>
#include <vector>

namespace xllm {

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
    std::string models_dir;
    std::vector<std::string> origin_allowlist;
    int node_port{32769};
    bool require_gpu{true};
    std::string bind_address{"0.0.0.0"};
    std::string default_embedding_model{"nomic-embed-text-v1.5"};
    std::string cli_model_path;   // Model path from --model CLI option
    std::string cli_model_name;   // Model name from --model-name CLI option
    std::string cli_mmproj_path;  // MMProj path from --mmproj CLI option
    int cli_ctx_size{0};          // Context size from --ctx-size CLI option (0 = use default)
    bool cors_enabled{true};
    std::string cors_allow_origin{"*"};
    std::string cors_allow_methods{"GET, POST, OPTIONS"};
    std::string cors_allow_headers{"Content-Type, Authorization"};
    bool gzip_enabled{true};
};

NodeConfig loadNodeConfig();
std::pair<NodeConfig, std::string> loadNodeConfigWithLog();

}  // namespace xllm
