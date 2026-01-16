#pragma once

#include <string>
#include <chrono>
#include <cstdint>

namespace llm_node {
namespace cli {

/// Progress renderer for download operations (ollama-style)
class ProgressRenderer {
public:
    /// Constructor
    /// @param total_bytes Total bytes to download (0 if unknown)
    explicit ProgressRenderer(uint64_t total_bytes = 0);

    ~ProgressRenderer();

    /// Update progress
    /// @param downloaded_bytes Bytes downloaded so far
    /// @param speed_bps Current download speed in bytes/second
    void update(uint64_t downloaded_bytes, double speed_bps);

    /// Mark as completed
    void complete();

    /// Mark as failed
    /// @param error_message Error message to display
    void fail(const std::string& error_message);

    /// Set the current phase/step name
    /// @param phase Phase name (e.g., "pulling manifest", "pulling abc123...")
    void setPhase(const std::string& phase);

    /// Get progress bar string
    /// @param downloaded_bytes Bytes downloaded
    /// @param total_bytes Total bytes
    /// @param width Width of progress bar
    /// @return Progress bar string (e.g., "45% [======    ]")
    static std::string formatProgressBar(uint64_t downloaded_bytes, uint64_t total_bytes, int width = 20);

    /// Format bytes as human-readable string
    /// @param bytes Number of bytes
    /// @return Human-readable string (e.g., "6.4 GB", "128 MB")
    static std::string formatBytes(uint64_t bytes);

    /// Format speed as human-readable string
    /// @param bps Speed in bytes per second
    /// @return Human-readable string (e.g., "45.2 MB/s")
    static std::string formatSpeed(double bps);

    /// Format duration as human-readable string
    /// @param seconds Duration in seconds
    /// @return Human-readable string (e.g., "2m 30s", "45s")
    static std::string formatDuration(double seconds);

private:
    uint64_t total_bytes_;
    uint64_t downloaded_bytes_;
    std::string phase_;
    std::chrono::steady_clock::time_point start_time_;
    bool completed_;
    bool failed_;

    /// Clear current line and print new content
    void clearAndPrint(const std::string& content);
};

}  // namespace cli
}  // namespace llm_node
