#include <gtest/gtest.h>
#include <spdlog/sinks/ostream_sink.h>
#include <spdlog/spdlog.h>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <unordered_map>
#include <vector>

#include "utils/allowlist.h"
#include "utils/json_utils.h"
#include "utils/logger.h"
#include "utils/system_info.h"

using namespace xllm;
namespace fs = std::filesystem;

class EnvGuard {
public:
    explicit EnvGuard(std::vector<std::string> keys) : keys_(std::move(keys)) {
        for (const auto& key : keys_) {
            const char* value = std::getenv(key.c_str());
            if (value) {
                saved_[key] = value;
            }
        }
    }
    ~EnvGuard() {
        for (const auto& key : keys_) {
            if (auto it = saved_.find(key); it != saved_.end()) {
                setenv(key.c_str(), it->second.c_str(), 1);
            } else {
                unsetenv(key.c_str());
            }
        }
    }

private:
    std::vector<std::string> keys_;
    std::unordered_map<std::string, std::string> saved_;
};

class TempDirGuard {
public:
    TempDirGuard() {
        auto base = fs::temp_directory_path() / fs::path("xllm-log-XXXXXX");
        std::string tmpl = base.string();
        std::vector<char> buf(tmpl.begin(), tmpl.end());
        buf.push_back('\0');
        char* created = mkdtemp(buf.data());
        path = created ? fs::path(created) : fs::temp_directory_path();
    }
    ~TempDirGuard() {
        std::error_code ec;
        fs::remove_all(path, ec);
    }

    fs::path path;
};

TEST(LoggerTest, InitSetsLevelAndWritesToSink) {
    auto original_logger = spdlog::default_logger();
    auto original_level = spdlog::get_level();
    std::stringstream ss;
    auto sink = std::make_shared<spdlog::sinks::ostream_sink_mt>(ss);
    xllm::logger::init("debug", "%v", "", {sink});

    spdlog::info("hello");
    EXPECT_EQ(spdlog::default_logger()->level(), spdlog::level::debug);
    auto output = ss.str();
    EXPECT_NE(output.find("hello"), std::string::npos);

    // Restore default logger to avoid dangling stream sinks in later tests.
    spdlog::set_default_logger(std::move(original_logger));
    spdlog::set_level(original_level);
    spdlog::drop("xllm");
}

TEST(LoggerTest, InitFromEnvWritesJsonLogsToFile) {
    auto original_logger = spdlog::default_logger();
    auto original_level = spdlog::get_level();

    EnvGuard env_guard({
        "XLLM_LOG_DIR",
        "XLLM_LOG_LEVEL",
        "XLLM_LOG_RETENTION_DAYS",
        "LLM_LOG_DIR",
        "LLM_LOG_LEVEL",
        "LLM_LOG_RETENTION_DAYS",
        "LOG_LEVEL"
    });

    TempDirGuard tmp;
    setenv("XLLM_LOG_DIR", tmp.path.string().c_str(), 1);
    setenv("XLLM_LOG_LEVEL", "info", 1);

    xllm::logger::init_from_env();
    spdlog::info("hello");
    spdlog::default_logger()->flush();

    std::string log_path = xllm::logger::get_log_file_path();

    spdlog::set_default_logger(std::move(original_logger));
    spdlog::set_level(original_level);
    spdlog::drop("xllm");

    std::ifstream ifs(log_path);
    ASSERT_TRUE(ifs.is_open());

    std::string line;
    bool found = false;
    while (std::getline(ifs, line)) {
        if (line.find("hello") != std::string::npos) {
            found = true;
            EXPECT_FALSE(line.empty());
            EXPECT_EQ(line.front(), '{');
            EXPECT_NE(line.find("\"level\""), std::string::npos);
            EXPECT_NE(line.find("\"msg\":\"hello\""), std::string::npos);
            break;
        }
    }

    EXPECT_TRUE(found);
}

TEST(JsonUtilsTest, ParseJsonHandlesInvalid) {
    std::string error;
    auto ok = parse_json(R"({"a":1})", &error);
    ASSERT_TRUE(ok.has_value());
    EXPECT_EQ(ok->at("a").get<int>(), 1);

    auto bad = parse_json("{invalid", &error);
    EXPECT_FALSE(bad.has_value());
    EXPECT_FALSE(error.empty());
}

TEST(JsonUtilsTest, HasRequiredKeysAndFallbacks) {
    nlohmann::json j = {{"name", "node"}, {"port", 32768}};
    std::string missing;
    EXPECT_TRUE(has_required_keys(j, {"name", "port"}, &missing));
    EXPECT_TRUE(missing.empty());

    EXPECT_FALSE(has_required_keys(j, {"name", "port", "host"}, &missing));
    EXPECT_EQ(missing, "host");

    EXPECT_EQ(get_or<int>(j, "port", 0), 32768);
    EXPECT_EQ(get_or<std::string>(j, "host", "localhost"), "localhost");
}

TEST(AllowlistTest, HuggingFaceHostMatchIsStrict) {
    const std::vector<std::string> allowlist = {"openai/*"};

    EXPECT_TRUE(isUrlAllowedByAllowlist(
        "https://huggingface.co/openai/gpt-oss/resolve/main/model.gguf", allowlist));
    EXPECT_FALSE(isUrlAllowedByAllowlist(
        "https://huggingface.co.evil.com/openai/gpt-oss/resolve/main/model.gguf", allowlist));
    EXPECT_FALSE(isUrlAllowedByAllowlist(
        "https://example.com/openai/gpt-oss/resolve/main/model.gguf", allowlist));
}

TEST(SystemInfoTest, CollectProvidesBasicInfo) {
    auto info = collect_system_info();
    EXPECT_FALSE(info.os.empty());
    EXPECT_FALSE(info.arch.empty());
    EXPECT_GT(info.cpu_cores, 0u);
    // Some platforms may not expose total memory; allow zero but prefer positive.
    EXPECT_GE(info.total_memory_bytes, 0u);

    auto summary = format_system_info(info);
    EXPECT_NE(summary.find("os="), std::string::npos);
    EXPECT_NE(summary.find("arch="), std::string::npos);
}
