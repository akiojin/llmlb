#include <gtest/gtest.h>
#include <cstdlib>
#include <array>
#include <memory>
#include <string>
#include <stdexcept>

#include "utils/cli.h"
#include "utils/version.h"

using namespace allm;

// Test --help flag
TEST(CliTest, HelpFlagShowsHelpMessage) {
    std::vector<std::string> args = {"llm-router", "--help"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("llm-router") != std::string::npos);
    EXPECT_TRUE(result.output.find("COMMANDS") != std::string::npos);
}

TEST(CliTest, ShortHelpFlagShowsHelpMessage) {
    std::vector<std::string> args = {"llm-router", "-h"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("llm-router") != std::string::npos);
}

// Test --version flag
TEST(CliTest, VersionFlagShowsVersion) {
    std::vector<std::string> args = {"llm-node", "--version"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find(ALLM_VERSION) != std::string::npos);
}

TEST(CliTest, ShortVersionFlagShowsVersion) {
    std::vector<std::string> args = {"llm-node", "-V"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find(ALLM_VERSION) != std::string::npos);
}

// Test no arguments (should continue to server mode)
TEST(CliTest, NoArgumentsContinuesToServerMode) {
    std::vector<std::string> args = {"llm-node"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_FALSE(result.should_exit);
}

// Test unknown argument (shows help with commands)
TEST(CliTest, UnknownArgumentShowsHelpOrError) {
    std::vector<std::string> args = {"llm-router", "--unknown-flag"};
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

// Test node subcommand help contains environment variables
TEST(CliTest, NodeHelpMessageContainsEnvironmentVariables) {
    std::vector<std::string> args = {"llm-router", "node", "--help"};
    std::vector<char*> argv;
    for (auto& s : args) argv.push_back(s.data());
    argv.push_back(nullptr);

    CliResult result = parseCliArgs(static_cast<int>(args.size()), argv.data());

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_TRUE(result.output.find("ALLM_MODELS_DIR") != std::string::npos);
    EXPECT_TRUE(result.output.find("ALLM_PORT") != std::string::npos);
}
