// SPEC-58378000: Contract tests for 'node rm' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace llm_node;

class CliRmTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
    }
};

// Contract: node rm requires a model name
TEST_F(CliRmTest, RequiresModelName) {
    const char* argv[] = {"llm-router", "node", "rm"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

// Contract: node rm parses model name
TEST_F(CliRmTest, ParseModelName) {
    const char* argv[] = {"llm-router", "node", "rm", "llama3.2"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::NodeRm);
    EXPECT_EQ(result.model_options.model, "llama3.2");
}

// Contract: node rm --help shows usage
TEST_F(CliRmTest, ShowHelp) {
    const char* argv[] = {"llm-router", "node", "rm", "--help"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

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
