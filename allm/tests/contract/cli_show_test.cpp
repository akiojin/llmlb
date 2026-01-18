// SPEC-58378000: Contract tests for 'show' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace llm_node;

class CliShowTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLM_ROUTER_HOST");
    }
};

// Contract: show requires a model name
TEST_F(CliShowTest, RequiresModelName) {
    const char* argv[] = {"allm", "show"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

// Contract: show parses model name
TEST_F(CliShowTest, ParseModelName) {
    const char* argv[] = {"allm", "show", "llama3.2"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Show);
    EXPECT_EQ(result.show_options.model, "llama3.2");
}

// Contract: show --license shows license only
TEST_F(CliShowTest, ParseLicenseFlag) {
    const char* argv[] = {"allm", "show", "llama3.2", "--license"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Show);
    EXPECT_TRUE(result.show_options.license_only);
}

// Contract: show --modelfile shows modelfile only
TEST_F(CliShowTest, ParseModelfileFlag) {
    const char* argv[] = {"allm", "show", "llama3.2", "--modelfile"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_TRUE(result.show_options.modelfile_only);
}

// Contract: show --parameters shows parameters only
TEST_F(CliShowTest, ParseParametersFlag) {
    const char* argv[] = {"allm", "show", "llama3.2", "--parameters"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_TRUE(result.show_options.parameters_only);
}

// Contract: show --template shows template only
TEST_F(CliShowTest, ParseTemplateFlag) {
    const char* argv[] = {"allm", "show", "llama3.2", "--template"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_TRUE(result.show_options.template_only);
}

// Contract: show --system shows system prompt only
TEST_F(CliShowTest, ParseSystemFlag) {
    const char* argv[] = {"allm", "show", "llama3.2", "--system"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_TRUE(result.show_options.system_only);
}

// Contract: show --help shows usage
TEST_F(CliShowTest, ShowHelp) {
    const char* argv[] = {"allm", "show", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("show"), std::string::npos);
}

// Contract: node show for ollama model shows read-only info
TEST_F(CliShowTest, DISABLED_ShowsOllamaModelInfo) {
    // Shows "Source: ollama (read-only)" for ollama: prefixed models
    EXPECT_TRUE(false);
}

// Contract: node show includes HuggingFace metadata when available
TEST_F(CliShowTest, DISABLED_IncludesHuggingFaceMetadata) {
    // Shows repo_id, author, downloads, likes when available
    EXPECT_TRUE(false);
}

// Contract: node show returns exit code 1 if model not found
TEST_F(CliShowTest, DISABLED_ReturnsErrorIfModelNotFound) {
    EXPECT_TRUE(false);
}
