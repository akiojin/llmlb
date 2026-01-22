// SPEC-58378000: Contract tests for 'rm' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliRmTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

// Contract: rm requires a model name
TEST_F(CliRmTest, RequiresModelName) {
    const char* argv[] = {"xllm", "rm"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

// Contract: rm parses model name
TEST_F(CliRmTest, ParseModelName) {
    const char* argv[] = {"xllm", "rm", "llama3.2"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Rm);
    EXPECT_EQ(result.model_options.model, "llama3.2");
}

// Contract: rm --help shows usage
TEST_F(CliRmTest, ShowHelp) {
    const char* argv[] = {"xllm", "rm", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("rm"), std::string::npos);
}

// Contract: node rm deletes immediately without confirmation (ollama compatible)
TEST_F(CliRmTest, DISABLED_DeletesWithoutConfirmation) {
    // ollama rm does not ask for confirmation
    EXPECT_TRUE(false);
}

// Contract: node rm returns error for ollama models
TEST_F(CliRmTest, DISABLED_ReturnsErrorForOllamaModels) {
    // ollama: prefixed models are read-only, cannot be deleted
    // Should show: "Use 'ollama rm <model>' to delete"
    EXPECT_TRUE(false);
}

// Contract: node rm returns exit code 1 if model not found
TEST_F(CliRmTest, DISABLED_ReturnsErrorIfModelNotFound) {
    EXPECT_TRUE(false);
}

// Contract: node rm prints "deleted '<model>'" on success
TEST_F(CliRmTest, DISABLED_PrintsDeletedOnSuccess) {
    // Output format: deleted 'llama3.2'
    EXPECT_TRUE(false);
}
