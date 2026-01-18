// SPEC-58378000: DownloadProgress data model
// Represents download progress information

#pragma once

#include <string>
#include <optional>
#include <cstdint>

namespace allm {
namespace cli {
namespace models {

/// Download status
enum class DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed
};

/// Download progress information
struct DownloadProgress {
    /// Model name being downloaded
    std::string model_name;

    /// Total bytes to download
    uint64_t total_bytes{0};

    /// Bytes downloaded so far
    uint64_t downloaded_bytes{0};

    /// Current download speed in bytes per second
    uint64_t speed_bps{0};

    /// Estimated time remaining in seconds
    std::optional<uint32_t> eta_secs;

    /// Current status
    DownloadStatus status{DownloadStatus::Pending};

    /// Error message (if status is Failed)
    std::string error_message;

    /// Get progress percentage (0-100)
    float getProgressPercent() const;

    /// Format progress for display (e.g., "45%")
    std::string formatProgress() const;

    /// Format speed for display (e.g., "25.3 MB/s")
    std::string formatSpeed() const;

    /// Format ETA for display (e.g., "2m 30s")
    std::string formatEta() const;

    /// Format downloaded/total size (e.g., "2.5 GB / 6.4 GB")
    std::string formatSize() const;

    /// Get status as string
    static std::string statusToString(DownloadStatus status);

    /// Check if download is complete
    bool isComplete() const { return status == DownloadStatus::Completed; }

    /// Check if download failed
    bool isFailed() const { return status == DownloadStatus::Failed; }

    /// Check if download is in progress
    bool isInProgress() const { return status == DownloadStatus::Downloading; }
};

}  // namespace models
}  // namespace cli
}  // namespace allm
