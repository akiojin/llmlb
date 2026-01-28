#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliConvertTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

TEST_F(CliConvertTest, RequiresSourcePath) {
    const char* argv[] = {"xllm", "convert"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("source"), std::string::npos);
}

TEST_F(CliConvertTest, RequiresName) {
    const char* argv[] = {"xllm", "convert", "model.safetensors"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("name"), std::string::npos);
}

TEST_F(CliConvertTest, ParsesSourceAndName) {
    const char* argv[] = {"xllm", "convert", "model.safetensors", "--name", "llama3"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Convert);
    EXPECT_EQ(result.convert_options.source, "model.safetensors");
    EXPECT_EQ(result.convert_options.name, "llama3");
}

TEST_F(CliConvertTest, ShowHelp) {
    const char* argv[] = {"xllm", "convert", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_NE(result.output.find("convert"), std::string::npos);
}
