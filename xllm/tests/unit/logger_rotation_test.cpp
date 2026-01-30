#include <gtest/gtest.h>

#include <chrono>
#include <filesystem>
#include <iomanip>
#include <sstream>
#include <vector>
#include <fstream>

#include "utils/logger.h"

namespace fs = std::filesystem;

namespace {
class TempDir {
public:
    TempDir() {
        auto base = fs::temp_directory_path() / fs::path("log-rotation-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }

    ~TempDir() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }

    fs::path path;
};

std::string format_date(std::chrono::system_clock::time_point tp) {
    auto t = std::chrono::system_clock::to_time_t(tp);
    std::tm tm{};
#ifdef _WIN32
    localtime_s(&tm, &t);
#else
    localtime_r(&t, &tm);
#endif
    std::ostringstream oss;
    oss << std::put_time(&tm, "%Y-%m-%d");
    return oss.str();
}

void touch_file(const fs::path& path) {
    std::ofstream ofs(path);
    ofs << "log";
}
}  // namespace

TEST(LoggerRotationTest, RemovesLogsOlderThanRetention) {
    TempDir temp;

    auto now = std::chrono::system_clock::now();
    auto old_date = now - std::chrono::hours(24 * 10);
    auto recent_date = now - std::chrono::hours(24);

    fs::path old_file = temp.path / ("xllm.jsonl." + format_date(old_date));
    fs::path recent_file = temp.path / ("xllm.jsonl." + format_date(recent_date));
    touch_file(old_file);
    touch_file(recent_file);

    xllm::logger::cleanup_old_logs(temp.path.string(), 3);

    EXPECT_FALSE(fs::exists(old_file));
    EXPECT_TRUE(fs::exists(recent_file));
}
