// SPEC-58378000: Contract tests for 'run' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace llm_node;

class CliRunTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
        unsetenv("LLM_NODE_PORT");
    }
};

// Contract: run requires a model name
TEST_F(CliRunTest, RequiresModelName) {
    const char* argv[] = {"allm", "run"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

// Contract: run parses model name correctly
TEST_F(CliRunTest, ParseModelName) {
    const char* argv[] = {"allm", "run", "llama3.2"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Run);
    EXPECT_EQ(result.run_options.model, "llama3.2");
}

// Contract: run accepts --think flag for reasoning models
TEST_F(CliRunTest, ParseThinkFlag) {
    const char* argv[] = {"allm", "run", "deepseek-r1", "--think"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Run);
    EXPECT_EQ(result.run_options.model, "deepseek-r1");
    EXPECT_TRUE(result.run_options.show_thinking);
}

// Contract: run accepts --hide-think flag (default)
TEST_F(CliRunTest, ParseHideThinkFlag) {
    const char* argv[] = {"allm", "run", "deepseek-r1", "--hide-think"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Run);
    EXPECT_TRUE(result.run_options.hide_thinking);
    EXPECT_FALSE(result.run_options.show_thinking);
}

// Contract: run --help shows usage
TEST_F(CliRunTest, ShowHelp) {
    const char* argv[] = {"allm", "run", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("run"), std::string::npos);
}

// Contract: run accepts model with tag (e.g., llama3.2:latest)
TEST_F(CliRunTest, ParseModelWithTag) {
    const char* argv[] = {"allm", "run", "llama3.2:latest"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.run_options.model, "llama3.2:latest");
}

// Contract: run accepts ollama-prefixed model
TEST_F(CliRunTest, ParseOllamaModel) {
    const char* argv[] = {"allm", "run", "ollama:llama3.2"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.run_options.model, "ollama:llama3.2");
}

// Contract: node run returns exit code 2 if server not running
// This is a functional contract verified in integration tests
TEST_F(CliRunTest, DISABLED_ReturnsConnectionErrorIfServerDown) {
    // This test requires server interaction
    // Exit code 2 = connection error
    EXPECT_TRUE(false);
}

// Contract: /bye command in REPL exits with code 0
TEST_F(CliRunTest, DISABLED_ByeCommandExitsCleanly) {
    // This test requires REPL interaction
    EXPECT_TRUE(false);
}

// Contract: /clear command in REPL clears history
TEST_F(CliRunTest, DISABLED_ClearCommandClearsHistory) {
    // This test requires REPL interaction
    EXPECT_TRUE(false);
}
