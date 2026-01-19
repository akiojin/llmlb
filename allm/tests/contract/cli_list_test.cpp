// SPEC-58378000: Contract tests for 'list' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace allm;

class CliListTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
    }
};

// Contract: list requires no arguments
TEST_F(CliListTest, ParseNoArguments) {
    const char* argv[] = {"allm", "list"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::List);
}

// Contract: list --help shows usage
TEST_F(CliListTest, ShowHelp) {
    const char* argv[] = {"allm", "list", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("list"), std::string::npos);
}

// Contract: node list output includes NAME, ID, SIZE, MODIFIED columns
// Format matches ollama list output
TEST_F(CliListTest, DISABLED_OutputFormat) {
    // This test requires server interaction and output capture
    // Expected columns: NAME (40), ID (20), SIZE (12), MODIFIED (20)
    EXPECT_TRUE(false);
}

// Contract: node list shows ollama models with "ollama:" prefix
TEST_F(CliListTest, DISABLED_ShowsOllamaModelsWithPrefix) {
    // When ~/.ollama/models/ contains models, they should appear
    // with "ollama:" prefix and "(readonly)" in MODIFIED column
    EXPECT_TRUE(false);
}

// Contract: node list returns exit code 2 if server not running
TEST_F(CliListTest, DISABLED_ReturnsConnectionErrorIfServerDown) {
    // This test requires server interaction
    EXPECT_TRUE(false);
}

// Contract: node list returns exit code 0 even if no models
TEST_F(CliListTest, DISABLED_ReturnsZeroIfNoModels) {
    // Empty list is not an error
    EXPECT_TRUE(false);
}
