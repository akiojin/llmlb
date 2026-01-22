// SPEC-58378000: Contract tests for 'ps' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliPsTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
    }
};

// Contract: ps requires no arguments
TEST_F(CliPsTest, ParseNoArguments) {
    const char* argv[] = {"allm", "ps"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Ps);
}

// Contract: ps --help shows usage
TEST_F(CliPsTest, ShowHelp) {
    const char* argv[] = {"allm", "ps", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("ps"), std::string::npos);
}

// Contract: node ps output includes ollama-compatible columns + extensions
// Columns: NAME, ID, SIZE, PROCESSOR, VRAM, TEMP, REQS, UNTIL
TEST_F(CliPsTest, DISABLED_OutputFormat) {
    // ollama-compatible: NAME, ID, SIZE, PROCESSOR, UNTIL
    // Extended: VRAM (usage %), TEMP (GPU °C), REQS (request count)
    EXPECT_TRUE(false);
}

// Contract: node ps shows VRAM usage percentage
TEST_F(CliPsTest, DISABLED_ShowsVramUsage) {
    // Format: "85%", "12%", etc.
    EXPECT_TRUE(false);
}

// Contract: node ps shows GPU temperature
TEST_F(CliPsTest, DISABLED_ShowsGpuTemperature) {
    // Format: "65°C", "-" if unavailable
    EXPECT_TRUE(false);
}

// Contract: node ps shows request count
TEST_F(CliPsTest, DISABLED_ShowsRequestCount) {
    // Number of requests served by this model instance
    EXPECT_TRUE(false);
}

// Contract: node ps returns exit code 2 if server not running
TEST_F(CliPsTest, DISABLED_ReturnsConnectionErrorIfServerDown) {
    EXPECT_TRUE(false);
}

// Contract: node ps returns exit code 0 even if no models running
TEST_F(CliPsTest, DISABLED_ReturnsZeroIfNoModelsRunning) {
    // Empty list is not an error, just shows header
    EXPECT_TRUE(false);
}
