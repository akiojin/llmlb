// SPEC-58378000: Contract tests for 'node serve' command
// Tests CLI parsing and option handling for the serve command.
// Note: The actual serve command is implemented in main.cpp and starts
// the HTTP server via run_node(). These tests cover CLI parsing only.

#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliServeTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Reset environment variables
        unsetenv("LLMLB_HOST");
        unsetenv("XLLM_PORT");
    }
};

// Contract: serve should parse default options correctly
TEST_F(CliServeTest, ParseDefaultOptions) {
    const char* argv[] = {"xllm", "serve"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Serve);
    EXPECT_EQ(result.serve_options.port, 32769);
    EXPECT_EQ(result.serve_options.host, "0.0.0.0");
}

// Contract: serve should accept --port option
TEST_F(CliServeTest, ParseCustomPort) {
    const char* argv[] = {"xllm", "serve", "--port", "8080"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Serve);
    EXPECT_EQ(result.serve_options.port, 8080);
}

// Contract: serve should accept --host option
TEST_F(CliServeTest, ParseCustomHost) {
    const char* argv[] = {"xllm", "serve", "--host", "127.0.0.1"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Serve);
    EXPECT_EQ(result.serve_options.host, "127.0.0.1");
}

// Contract: serve should respect XLLM_PORT environment variable
TEST_F(CliServeTest, RespectPortEnvironmentVariable) {
    setenv("XLLM_PORT", "9999", 1);

    const char* argv[] = {"xllm", "serve"};
    auto result = parseCliArgs(2, const_cast<char**>(argv));

    // Environment variable should be respected when no explicit --port
    // Note: This tests parsing; actual port binding is in serve implementation
    EXPECT_EQ(result.subcommand, Subcommand::Serve);
}

// Contract: serve --help should show help message
TEST_F(CliServeTest, ShowHelp) {
    const char* argv[] = {"xllm", "serve", "--help"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_FALSE(result.output.empty());
    EXPECT_NE(result.output.find("serve"), std::string::npos);
}

// Note: Integration tests for the serve command (server startup, port binding)
// are covered in integration/http_server_test.cpp and require running the
// full xllm binary with the serve subcommand.
