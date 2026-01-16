// SPEC-58378000: Node data model
// Represents an inference server node

#pragma once

#include <string>
#include <vector>
#include <optional>
#include <cstdint>

namespace llm_node {
namespace cli {
namespace models {

/// Node status
enum class NodeStatus {
    Running,
    Stopped,
    Error
};

/// Inference server node
struct NodeInfo {
    /// Node ID (UUID)
    std::string id;

    /// Host name or IP address
    std::string host;

    /// Port number
    uint16_t port{0};

    /// Current status
    NodeStatus status{NodeStatus::Stopped};

    /// List of loaded model names
    std::vector<std::string> loaded_models;

    /// Total VRAM in bytes
    uint64_t vram_total_bytes{0};

    /// Used VRAM in bytes
    uint64_t vram_used_bytes{0};

    /// GPU temperature in Celsius
    std::optional<float> gpu_temperature;

    /// Uptime in seconds
    uint64_t uptime_secs{0};

    /// Get VRAM usage percentage (0-100)
    float getVramUsagePercent() const;

    /// Format VRAM for display (e.g., "8.5 GB / 12.0 GB (71%)")
    std::string formatVram() const;

    /// Format uptime for display (e.g., "2h 30m")
    std::string formatUptime() const;

    /// Format temperature for display (e.g., "62°C")
    std::string formatTemperature() const;

    /// Get status as string
    static std::string statusToString(NodeStatus status);
};

}  // namespace models
}  // namespace cli
}  // namespace llm_node
