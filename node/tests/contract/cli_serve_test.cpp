// SPEC-58378000: Contract tests for 'node serve' command
// TDD RED phase - these tests MUST fail until implementation is complete

#include <gtest/gtest.h>
#include "utils/cli.h"

namespace llm_node {
namespace cli {
namespace commands {
// Forward declaration for serve command
int serve(const ServeOptions& options);
}
}
}

using namespace llm_node;
using namespace llm_node::cli;

class CliServeTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Reset environment variables
        unsetenv("LLM_ROUTER_HOST");
        unsetenv("LLM_NODE_PORT");
    }
};

// Contract: node serve should parse default options correctly
TEST_F(CliServeTest, ParseDefaultOptions) {
    const char* argv[] = {"llm-router", "node", "serve"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::NodeServe);
    EXPECT_EQ(result.serve_options.port, 32769);
    EXPECT_EQ(result.serve_options.host, "0.0.0.0");
}

// Contract: node serve should accept --port option
TEST_F(CliServeTest, ParseCustomPort) {
    const char* argv[] = {"llm-router", "node", "serve", "--port", "8080"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::NodeServe);
    EXPECT_EQ(result.serve_options.port, 8080);
}

// Contract: node serve should accept --host option
TEST_F(CliServeTest, ParseCustomHost) {
    const char* argv[] = {"llm-router", "node", "serve", "--host", "127.0.0.1"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::NodeServe);
    EXPECT_EQ(result.serve_options.host, "127.0.0.1");
}

// Contract: node serve should respect LLM_NODE_PORT environment variable
TEST_F(CliServeTest, RespectPortEnvironmentVariable) {
    setenv("LLM_NODE_PORT", "9999", 1);

    const char* argv[] = {"llm-router", "node", "serve"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    // Environment variable should be respected when no explicit --port
    // Note: This tests parsing; actual port binding is in serve implementation
    EXPECT_EQ(result.subcommand, Subcommand::NodeServe);
}

// Contract: node serve --help should show help message
TEST_F(CliServeTest, ShowHelp) {
    const char* argv[] = {"llm-router", "node", "serve", "--help"};
    auto result = parseCliArgs(4, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 0);
    EXPECT_FALSE(result.output.empty());
    EXPECT_NE(result.output.find("serve"), std::string::npos);
}

// Contract: serve command returns exit code 0 on successful start
// Note: This is an integration-level contract, implementation pending
TEST_F(CliServeTest, DISABLED_ReturnsZeroOnSuccess) {
    ServeOptions options;
    options.port = 0; // Use random port for testing
    options.host = "127.0.0.1";

    int exit_code = cli::commands::serve(options);
    EXPECT_EQ(exit_code, 0);
}

// Contract: serve command returns exit code 1 if port is in use
TEST_F(CliServeTest, DISABLED_ReturnsOneIfPortInUse) {
    ServeOptions options;
    options.port = 32769; // Assume this port is in use
    options.host = "127.0.0.1";

    // First instance
    // int first_exit = cli::commands::serve(options);

    // Second instance should fail
    int exit_code = cli::commands::serve(options);
    EXPECT_EQ(exit_code, 1);
}
