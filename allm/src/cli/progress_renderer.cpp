#include "cli/progress_renderer.h"
#include <iostream>
#include <iomanip>
#include <sstream>
#include <cmath>
#include <algorithm>

namespace allm {
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

std::string ProgressRenderer::formatProgressBarBare(uint64_t downloaded_bytes, uint64_t total_bytes, int width) {
    if (total_bytes == 0) {
        return "";
    }

    double progress = static_cast<double>(downloaded_bytes) / static_cast<double>(total_bytes);
    int filled = static_cast<int>(progress * width);

    std::ostringstream oss;
    oss << "[";
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

MultiProgressRenderer::MultiProgressRenderer()
    : start_time_(std::chrono::steady_clock::now()) {}

void MultiProgressRenderer::onManifest(size_t total_files) {
    total_files_ = total_files;
    render();
}

void MultiProgressRenderer::onProgress(const std::string& file, uint64_t completed, uint64_t total) {
    auto it = files_.find(file);
    if (it == files_.end()) {
        FileState state;
        state.started = std::chrono::steady_clock::now();
        state.completed = completed;
        state.total = total;
        state.status = "Downloading";
        files_.emplace(file, state);
        order_.push_back(file);
    } else {
        it->second.completed = completed;
        it->second.total = total;
        if (!it->second.done) {
            it->second.status = "Downloading";
        }
    }
    render();
}

void MultiProgressRenderer::onComplete(const std::string& file, const std::string& status) {
    auto it = files_.find(file);
    if (it == files_.end()) {
        FileState state;
        state.started = std::chrono::steady_clock::now();
        state.status = status == "downloaded" ? "Pull complete" : "Pull failed";
        state.done = true;
        files_.emplace(file, state);
        order_.push_back(file);
    } else {
        it->second.status = status == "downloaded" ? "Pull complete" : "Pull failed";
        it->second.done = true;
    }
    render();
}

void MultiProgressRenderer::onDone(const std::string& status) {
    final_status_ = status;
    render();
    std::cout << std::endl;
}

void MultiProgressRenderer::render() {
    std::vector<std::string> lines;

    size_t done = 0;
    for (const auto& entry : files_) {
        if (entry.second.done) {
            ++done;
        }
    }
    size_t total = total_files_ > 0 ? total_files_ : files_.size();

    {
        std::ostringstream oss;
        oss << "[+] Running " << done << "/" << total;
        lines.push_back(oss.str());
    }

    for (const auto& file : order_) {
        const auto& state = files_.at(file);
        std::ostringstream oss;
        const std::string icon = state.done ? "✔" : spinnerFrame();

        oss << " " << icon << " " << shortenFileLabel(file) << " ";
        oss << std::left << std::setw(13) << state.status;

        if (!state.done && state.total > 0) {
            oss << " " << ProgressRenderer::formatProgressBarBare(state.completed, state.total, 60);
            oss << "  " << formatBytesCompact(state.completed)
                << "/" << formatBytesCompact(state.total);
        }

        auto now = std::chrono::steady_clock::now();
        auto elapsed = std::chrono::duration_cast<std::chrono::duration<double>>(now - state.started).count();
        oss << " " << formatDurationShort(elapsed);

        lines.push_back(oss.str());
    }

    if (!final_status_.empty() && final_status_ != "ok") {
        lines.push_back("Status: " + final_status_);
    }

    renderLines(lines);
    spinner_index_++;
}

std::string MultiProgressRenderer::shortenFileLabel(const std::string& file) const {
    auto pos = file.find_last_of('/');
    std::string label = pos == std::string::npos ? file : file.substr(pos + 1);
    if (label.size() <= 28) {
        return label;
    }
    return label.substr(0, 12) + "…" + label.substr(label.size() - 12);
}

std::string MultiProgressRenderer::formatBytesCompact(uint64_t bytes) {
    const char* units[] = {"B", "KB", "MB", "GB", "TB"};
    int unit_index = 0;
    double size = static_cast<double>(bytes);

    while (size >= 1024.0 && unit_index < 4) {
        size /= 1024.0;
        ++unit_index;
    }

    std::ostringstream oss;
    if (unit_index == 0) {
        oss << static_cast<uint64_t>(size) << units[unit_index];
    } else {
        oss << std::fixed << std::setprecision(2) << size << units[unit_index];
        auto str = oss.str();
        auto dot = str.find('.');
        if (dot != std::string::npos) {
            while (!str.empty() && str.back() == '0') {
                str.pop_back();
            }
            if (!str.empty() && str.back() == '.') {
                str.pop_back();
            }
        }
        return str;
    }

    return oss.str();
}

std::string MultiProgressRenderer::formatDurationShort(double seconds) {
    std::ostringstream oss;

    if (seconds < 60.0) {
        oss << std::fixed << std::setprecision(1) << seconds << "s";
        return oss.str();
    }

    return ProgressRenderer::formatDuration(seconds);
}

void MultiProgressRenderer::renderLines(const std::vector<std::string>& lines) {
    if (lines_rendered_ > 0) {
        std::cout << "\033[" << lines_rendered_ << "A";
    }
    for (size_t i = 0; i < lines_rendered_; ++i) {
        std::cout << "\033[2K";
        if (i + 1 < lines_rendered_) {
            std::cout << "\n";
        }
    }
    if (lines_rendered_ > 0) {
        std::cout << "\r";
    }
    for (size_t i = 0; i < lines.size(); ++i) {
        std::cout << lines[i];
        if (i + 1 < lines.size()) {
            std::cout << "\n";
        }
    }
    std::cout << std::flush;
    lines_rendered_ = lines.size();
}

std::string MultiProgressRenderer::spinnerFrame() const {
    static const char* frames[] = {"⠦", "⠧", "⠇", "⠏", "⠋", "⠙", "⠹", "⠸", "⠼", "⠴"};
    return frames[spinner_index_ % (sizeof(frames) / sizeof(frames[0]))];
}

}  // namespace cli
}  // namespace allm
