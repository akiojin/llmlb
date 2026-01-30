#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliProfileTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

TEST_F(CliProfileTest, RequiresModelName) {
    const char* argv[] = {"xllm", "profile"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

TEST_F(CliProfileTest, ParsesModelName) {
    const char* argv[] = {"xllm", "profile", "llama3"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Profile);
    EXPECT_EQ(result.profile_options.model, "llama3");
}

TEST_F(CliProfileTest, ParsesPromptAndTokens) {
    const char* argv[] = {"xllm", "profile", "llama3", "--prompt", "hi", "--tokens", "64"};
    auto result = parseCliArgs(7, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.profile_options.prompt, "hi");
    EXPECT_EQ(result.profile_options.max_tokens, 64);
}

TEST_F(CliProfileTest, ShowHelp) {
    const char* argv[] = {"xllm", "profile", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("profile"), std::string::npos);
}
