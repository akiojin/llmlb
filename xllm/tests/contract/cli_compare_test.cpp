#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliCompareTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

TEST_F(CliCompareTest, RequiresTwoModels) {
    const char* argv[] = {"xllm", "compare", "llama3"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

TEST_F(CliCompareTest, ParsesModels) {
    const char* argv[] = {"xllm", "compare", "llama3", "mistral"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Compare);
    EXPECT_EQ(result.compare_options.model_a, "llama3");
    EXPECT_EQ(result.compare_options.model_b, "mistral");
}

TEST_F(CliCompareTest, ShowHelp) {
    const char* argv[] = {"xllm", "compare", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("compare"), std::string::npos);
}
