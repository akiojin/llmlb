// SPEC-58378000: Contract tests for 'node stop' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliStopTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
    }
};

// Contract: stop requires a model name
TEST_F(CliStopTest, RequiresModelName) {
    const char* argv[] = {"allm", "stop"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

// Contract: stop parses model name
TEST_F(CliStopTest, ParseModelName) {
    const char* argv[] = {"allm", "stop", "llama3.2"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Stop);
    EXPECT_EQ(result.model_options.model, "llama3.2");
}

// Contract: stop --help shows usage
TEST_F(CliStopTest, ShowHelp) {
    const char* argv[] = {"allm", "stop", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("stop"), std::string::npos);
}

// Contract: node stop unloads model from VRAM
TEST_F(CliStopTest, DISABLED_UnloadsModelFromVram) {
    // Model should be unloaded, VRAM freed
    EXPECT_TRUE(false);
}

// Contract: node stop returns exit code 1 if model not running
TEST_F(CliStopTest, DISABLED_ReturnsErrorIfModelNotRunning) {
    EXPECT_TRUE(false);
}

// Contract: node stop returns exit code 2 if server not running
TEST_F(CliStopTest, DISABLED_ReturnsConnectionErrorIfServerDown) {
    EXPECT_TRUE(false);
}

// Contract: node stop prints success message
TEST_F(CliStopTest, DISABLED_PrintsSuccessMessage) {
    // Output: "Stopped model 'llama3.2'"
    EXPECT_TRUE(false);
}
