#include "cli/progress_renderer.h"
#include <iostream>
#include <iomanip>
#include <sstream>
#include <cmath>

namespace llm_node {
namespace cli {

ProgressRenderer::ProgressRenderer(uint64_t total_bytes)
    : total_bytes_(total_bytes)
    , downloaded_bytes_(0)
    , start_time_(std::chrono::steady_clock::now())
    , completed_(false)
    , failed_(false)
{
}

ProgressRenderer::~ProgressRenderer() = default;

void ProgressRenderer::update(uint64_t downloaded_bytes, double speed_bps) {
    if (completed_ || failed_) {
        return;
    }

    downloaded_bytes_ = downloaded_bytes;

    std::ostringstream oss;

    // Phase name
    if (!phase_.empty()) {
        oss << phase_ << " ";
    }

    // Progress bar (if total is known)
    if (total_bytes_ > 0) {
        oss << formatProgressBar(downloaded_bytes_, total_bytes_);
        oss << " ";
    }

    // Downloaded size
    oss << formatBytes(downloaded_bytes_);
    if (total_bytes_ > 0) {
        oss << "/" << formatBytes(total_bytes_);
    }

    // Speed
    if (speed_bps > 0) {
        oss << " " << formatSpeed(speed_bps);
    }

    // ETA (if total is known and speed > 0)
    if (total_bytes_ > 0 && speed_bps > 0 && downloaded_bytes_ < total_bytes_) {
        double remaining_bytes = static_cast<double>(total_bytes_ - downloaded_bytes_);
        double eta_seconds = remaining_bytes / speed_bps;
        oss << " ETA " << formatDuration(eta_seconds);
    }

    clearAndPrint(oss.str());
}

void ProgressRenderer::complete() {
    if (completed_ || failed_) {
        return;
    }

    completed_ = true;

    auto end_time = std::chrono::steady_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end_time - start_time_);
    double seconds = duration.count() / 1000.0;

    std::ostringstream oss;
    if (!phase_.empty()) {
        oss << phase_ << " ";
    }
    oss << "complete";

    if (total_bytes_ > 0) {
        oss << " " << formatBytes(total_bytes_);
    }

    if (seconds > 0) {
        oss << " in " << formatDuration(seconds);
    }

    clearAndPrint(oss.str());
    std::cout << std::endl;
}

void ProgressRenderer::fail(const std::string& error_message) {
    if (completed_ || failed_) {
        return;
    }

    failed_ = true;

    std::ostringstream oss;
    if (!phase_.empty()) {
        oss << phase_ << " ";
    }
    oss << "failed: " << error_message;

    clearAndPrint(oss.str());
    std::cout << std::endl;
}

void ProgressRenderer::setPhase(const std::string& phase) {
    phase_ = phase;
}

std::string ProgressRenderer::formatProgressBar(uint64_t downloaded_bytes, uint64_t total_bytes, int width) {
    if (total_bytes == 0) {
        return "";
    }

    double progress = static_cast<double>(downloaded_bytes) / static_cast<double>(total_bytes);
    int filled = static_cast<int>(progress * width);

    std::ostringstream oss;
    int percent = static_cast<int>(progress * 100);
    oss << std::setw(3) << percent << "% [";

    for (int i = 0; i < width; ++i) {
        if (i < filled) {
            oss << "=";
        } else if (i == filled) {
            oss << ">";
        } else {
            oss << " ";
        }
    }

    oss << "]";
    return oss.str();
}

std::string ProgressRenderer::formatBytes(uint64_t bytes) {
    const char* units[] = {"B", "KB", "MB", "GB", "TB"};
    int unit_index = 0;
    double size = static_cast<double>(bytes);

    while (size >= 1024.0 && unit_index < 4) {
        size /= 1024.0;
        ++unit_index;
    }

    std::ostringstream oss;
    if (unit_index == 0) {
        oss << static_cast<uint64_t>(size) << " " << units[unit_index];
    } else {
        oss << std::fixed << std::setprecision(1) << size << " " << units[unit_index];
    }

    return oss.str();
}

std::string ProgressRenderer::formatSpeed(double bps) {
    const char* units[] = {"B/s", "KB/s", "MB/s", "GB/s"};
    int unit_index = 0;
    double speed = bps;

    while (speed >= 1024.0 && unit_index < 3) {
        speed /= 1024.0;
        ++unit_index;
    }

    std::ostringstream oss;
    oss << std::fixed << std::setprecision(1) << speed << " " << units[unit_index];
    return oss.str();
}

std::string ProgressRenderer::formatDuration(double seconds) {
    std::ostringstream oss;

    if (seconds < 60) {
        oss << static_cast<int>(std::ceil(seconds)) << "s";
    } else if (seconds < 3600) {
        int minutes = static_cast<int>(seconds / 60);
        int secs = static_cast<int>(seconds) % 60;
        oss << minutes << "m " << secs << "s";
    } else {
        int hours = static_cast<int>(seconds / 3600);
        int minutes = (static_cast<int>(seconds) % 3600) / 60;
        oss << hours << "h " << minutes << "m";
    }

    return oss.str();
}

void ProgressRenderer::clearAndPrint(const std::string& content) {
    // Clear line and print new content (carriage return)
    std::cout << "\r" << content;

    // Pad with spaces to clear any remaining characters from previous output
    static size_t last_length = 0;
    if (content.length() < last_length) {
        for (size_t i = content.length(); i < last_length; ++i) {
            std::cout << " ";
        }
        std::cout << "\r" << content;
    }
    last_length = content.length();

    std::cout.flush();
}

}  // namespace cli
}  // namespace llm_node
