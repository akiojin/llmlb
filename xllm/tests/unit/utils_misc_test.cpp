#include <gtest/gtest.h>
#include <spdlog/sinks/ostream_sink.h>
#include <spdlog/spdlog.h>
#include <sstream>

#include "utils/allowlist.h"
#include "utils/json_utils.h"
#include "utils/logger.h"
#include "utils/system_info.h"

using namespace xllm;

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
