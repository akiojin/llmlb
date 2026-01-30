#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliBenchmarkTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

TEST_F(CliBenchmarkTest, RequiresModelName) {
    const char* argv[] = {"xllm", "benchmark"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("model"), std::string::npos);
}

TEST_F(CliBenchmarkTest, ParsesModelNameAndRuns) {
    const char* argv[] = {"xllm", "benchmark", "llama3", "--runs", "3"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Benchmark);
    EXPECT_EQ(result.benchmark_options.model, "llama3");
    EXPECT_EQ(result.benchmark_options.runs, 3);
}

TEST_F(CliBenchmarkTest, ShowHelp) {
    const char* argv[] = {"xllm", "benchmark", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("benchmark"), std::string::npos);
}
