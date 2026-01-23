#include <gtest/gtest.h>
#include <cstdlib>
#include <array>
#include <memory>
#include <string>
#include <stdexcept>

#include "utils/cli.h"
#include "utils/version.h"

using namespace xllm;

// Test --help flag
TEST(CliTest, HelpFlagShowsHelpMessage) {
    std::vector<std::string> args = {"xllm", "--help"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("xllm") != std::string::npos);
    EXPECT_TRUE(result.output.find("COMMANDS") != std::string::npos);
}

TEST(CliTest, ShortHelpFlagShowsHelpMessage) {
    std::vector<std::string> args = {"xllm", "-h"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("xllm") != std::string::npos);
}

// Test --version flag
TEST(CliTest, VersionFlagShowsVersion) {
    std::vector<std::string> args = {"xllm", "--version"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find(XLLM_VERSION) != std::string::npos);
}

TEST(CliTest, ShortVersionFlagShowsVersion) {
    std::vector<std::string> args = {"xllm", "-V"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find(XLLM_VERSION) != std::string::npos);
}

// Test no arguments (should continue to server mode)
TEST(CliTest, NoArgumentsContinuesToServerMode) {
    std::vector<std::string> args = {"xllm"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_FALSE(result.should_exit);
}

// Test unknown argument (shows help with commands)
TEST(CliTest, UnknownArgumentShowsHelpOrError) {
    std::vector<std::string> args = {"xllm", "--unknown-flag"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    // Unknown flag now shows help message (exit code 0) or error (exit code != 0)
    EXPECT_TRUE(result.output.find("COMMANDS") != std::string::npos ||
                result.output.find("unknown") != std::string::npos ||
                result.output.find("Unknown") != std::string::npos);
}

// Test serve subcommand help contains environment variables
TEST(CliTest, ServeHelpMessageContainsEnvironmentVariables) {
    std::vector<std::string> args = {"xllm", "serve", "--help"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("XLLM_MODELS_DIR") != std::string::npos);
    EXPECT_TRUE(result.output.find("XLLM_PORT") != std::string::npos);
}
