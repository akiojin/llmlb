// SPEC-58378000: Integration test for REPL workflow
// T013: Verifies run -> prompt -> /bye interaction flow

#include <gtest/gtest.h>
#include "cli/cli_client.h"
#include "cli/repl_session.h"

#include <cstdlib>
#include <sstream>
#include <memory>

namespace allm {
namespace cli {
namespace {

/// Integration test fixture for REPL workflow
class CliReplTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Save original environment
        const char* host = std::getenv("LLM_ROUTER_HOST");
        const char* port = std::getenv("ALLM_PORT");
        original_host_ = host ? host : "";
        original_port_ = port ? port : "";

        // Set test environment
        setenv("LLM_ROUTER_HOST", "127.0.0.1", 1);
        setenv("ALLM_PORT", "11435", 1);
    }

    void TearDown() override {
        // Restore original environment
        if (original_host_.empty()) {
            unsetenv("LLM_ROUTER_HOST");
        } else {
            setenv("LLM_ROUTER_HOST", original_host_.c_str(), 1);
        }
        if (original_port_.empty()) {
            unsetenv("ALLM_PORT");
        } else {
            setenv("ALLM_PORT", original_port_.c_str(), 1);
        }
    }

    std::string original_host_;
    std::string original_port_;
};

/// Test: /bye command exits REPL cleanly
/// Scenario: User types /bye and REPL returns 0
/// DISABLED: This test hangs in CI due to isServerRunning() socket timeout
TEST_F(CliReplTest, DISABLED_ByeCommandExitsCleanly) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // Create REPL session with test model
    SessionSettings settings;
    settings.stream = false;  // Disable streaming for test
    ReplSession session(client, "test-model", settings);

    // Process /bye command
    bool should_continue = session.processInput("/bye");

    // Should return false (exit)
    EXPECT_FALSE(should_continue);

    // History should still be intact (not cleared)
    // /bye doesn't add to history
    EXPECT_EQ(session.getHistory().size(), 0);
}

/// Test: /clear command clears conversation history
/// Scenario: User has conversation, types /clear, history is empty
/// DISABLED: This test hangs in CI due to isServerRunning() socket timeout
TEST_F(CliReplTest, DISABLED_ClearCommandClearsHistory) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    SessionSettings settings;
    settings.stream = false;
    ReplSession session(client, "test-model", settings);

    // Simulate adding a message (processInput would normally do this)
    // For this test, we verify the clearHistory method directly
    session.clearHistory();

    // History should be empty
    EXPECT_TRUE(session.getHistory().empty());

    // Process /clear command
    bool should_continue = session.processInput("/clear");

    // Should return true (continue session)
    EXPECT_TRUE(should_continue);

    // History should be cleared
    EXPECT_TRUE(session.getHistory().empty());
}

/// Test: session maintains conversation context
/// Scenario: Multiple messages share context
TEST_F(CliReplTest, SessionMaintainsContext) {
    auto client = std::make_shared<CliClient>();

    // This test validates session state management
    // without requiring actual server responses

    SessionSettings settings;
    settings.temperature = 0.5f;
    settings.top_p = 0.8f;
    settings.show_thinking = true;

    ReplSession session(client, "llama3.2", settings);

    // Verify settings are preserved
    EXPECT_EQ(session.getModelName(), "llama3.2");
    EXPECT_FLOAT_EQ(session.getSettings().temperature, 0.5f);
    EXPECT_FLOAT_EQ(session.getSettings().top_p, 0.8f);
    EXPECT_TRUE(session.getSettings().show_thinking);

    // Update settings
    SessionSettings new_settings = settings;
    new_settings.temperature = 0.9f;
    session.updateSettings(new_settings);

    EXPECT_FLOAT_EQ(session.getSettings().temperature, 0.9f);
}

/// Test: REPL handles connection error gracefully
/// Scenario: Server disconnects mid-session
/// DISABLED: This test hangs in CI due to socket timeout on invalid port
TEST_F(CliReplTest, DISABLED_HandlesConnectionError) {
    // Use invalid port to simulate connection failure
    setenv("ALLM_PORT", "59999", 1);

    auto client = std::make_shared<CliClient>();

    // Verify server is not reachable
    EXPECT_FALSE(client->isServerRunning());

    // Create session anyway
    SessionSettings settings;
    ReplSession session(client, "test-model", settings);

    // Processing input should handle the error gracefully
    // The actual behavior depends on implementation
    // but it should not crash
    EXPECT_NO_THROW({
        session.processInput("hello");
    });
}

/// Test: thinking flag is respected in session
/// Scenario: --think flag enables thinking display
TEST_F(CliReplTest, ThinkingFlagRespected) {
    auto client = std::make_shared<CliClient>();

    // Test with thinking enabled
    SessionSettings settings_think;
    settings_think.show_thinking = true;
    ReplSession session_think(client, "deepseek-r1", settings_think);
    EXPECT_TRUE(session_think.getSettings().show_thinking);

    // Test with thinking disabled (default)
    SessionSettings settings_no_think;
    settings_no_think.show_thinking = false;
    ReplSession session_no_think(client, "deepseek-r1", settings_no_think);
    EXPECT_FALSE(session_no_think.getSettings().show_thinking);
}

/// Test: streaming mode configuration
/// Scenario: Session respects stream setting
TEST_F(CliReplTest, StreamingModeConfiguration) {
    auto client = std::make_shared<CliClient>();

    // Test with streaming enabled (default)
    SessionSettings settings_stream;
    settings_stream.stream = true;
    ReplSession session_stream(client, "llama3.2", settings_stream);
    EXPECT_TRUE(session_stream.getSettings().stream);

    // Test with streaming disabled
    SessionSettings settings_no_stream;
    settings_no_stream.stream = false;
    ReplSession session_no_stream(client, "llama3.2", settings_no_stream);
    EXPECT_FALSE(session_no_stream.getSettings().stream);
}

/// Test: max tokens configuration
/// Scenario: Session respects max_tokens setting
TEST_F(CliReplTest, MaxTokensConfiguration) {
    auto client = std::make_shared<CliClient>();

    // Test with unlimited (default)
    SessionSettings settings_unlimited;
    settings_unlimited.max_tokens = 0;
    ReplSession session_unlimited(client, "llama3.2", settings_unlimited);
    EXPECT_EQ(session_unlimited.getSettings().max_tokens, 0);

    // Test with limit
    SessionSettings settings_limited;
    settings_limited.max_tokens = 4096;
    ReplSession session_limited(client, "llama3.2", settings_limited);
    EXPECT_EQ(session_limited.getSettings().max_tokens, 4096);
}

/// Test: model name with tag is preserved
/// Scenario: Model name llama3.2:latest is preserved
TEST_F(CliReplTest, ModelNameWithTagPreserved) {
    auto client = std::make_shared<CliClient>();

    ReplSession session(client, "llama3.2:latest", {});
    EXPECT_EQ(session.getModelName(), "llama3.2:latest");

    ReplSession session2(client, "ollama:llama3.2", {});
    EXPECT_EQ(session2.getModelName(), "ollama:llama3.2");
}

/// Test: REPL with actual server interaction
/// This test requires a running server with loaded model
TEST_F(CliReplTest, DISABLED_ActualServerInteraction) {
    auto client = std::make_shared<CliClient>();

    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    SessionSettings settings;
    settings.stream = true;
    ReplSession session(client, "llama3.2", settings);

    // Send a test prompt
    bool should_continue = session.processInput("Say hello in exactly 5 words.");
    EXPECT_TRUE(should_continue);

    // History should have user message (and possibly assistant response)
    EXPECT_GE(session.getHistory().size(), 1);

    // Exit cleanly
    should_continue = session.processInput("/bye");
    EXPECT_FALSE(should_continue);
}

}  // namespace
}  // namespace cli
}  // namespace allm
