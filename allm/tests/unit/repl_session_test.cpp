// SPEC-58378000: ReplSession unit tests

#include <gtest/gtest.h>
#include "cli/repl_session.h"
#include "cli/cli_client.h"
#include <memory>

using namespace llm_node::cli;

// Test SessionSettings defaults
TEST(SessionSettingsTest, DefaultValues) {
    SessionSettings settings;
    EXPECT_FLOAT_EQ(settings.temperature, 0.7f);
    EXPECT_FLOAT_EQ(settings.top_p, 0.9f);
    EXPECT_EQ(settings.max_tokens, 0);
    EXPECT_FALSE(settings.show_thinking);
    EXPECT_TRUE(settings.stream);
}

// Test SessionSettings customization
TEST(SessionSettingsTest, CustomValues) {
    SessionSettings settings;
    settings.temperature = 0.5f;
    settings.top_p = 0.8f;
    settings.max_tokens = 1024;
    settings.show_thinking = true;
    settings.stream = false;

    EXPECT_FLOAT_EQ(settings.temperature, 0.5f);
    EXPECT_FLOAT_EQ(settings.top_p, 0.8f);
    EXPECT_EQ(settings.max_tokens, 1024);
    EXPECT_TRUE(settings.show_thinking);
    EXPECT_FALSE(settings.stream);
}

// Test Message struct
TEST(MessageTest, UserMessage) {
    Message msg;
    msg.role = MessageRole::User;
    msg.content = "Hello, world!";

    EXPECT_EQ(msg.role, MessageRole::User);
    EXPECT_EQ(msg.content, "Hello, world!");
    EXPECT_TRUE(msg.images.empty());
    EXPECT_TRUE(msg.thinking.empty());
}

TEST(MessageTest, AssistantMessage) {
    Message msg;
    msg.role = MessageRole::Assistant;
    msg.content = "Hi there!";
    msg.thinking = "Let me think...";

    EXPECT_EQ(msg.role, MessageRole::Assistant);
    EXPECT_EQ(msg.content, "Hi there!");
    EXPECT_EQ(msg.thinking, "Let me think...");
}

TEST(MessageTest, SystemMessage) {
    Message msg;
    msg.role = MessageRole::System;
    msg.content = "You are a helpful assistant.";

    EXPECT_EQ(msg.role, MessageRole::System);
    EXPECT_EQ(msg.content, "You are a helpful assistant.");
}

TEST(MessageTest, VisionMessage) {
    Message msg;
    msg.role = MessageRole::User;
    msg.content = "What's in this image?";
    msg.images.push_back("base64encodedimage1");
    msg.images.push_back("base64encodedimage2");

    EXPECT_EQ(msg.images.size(), 2);
    EXPECT_EQ(msg.images[0], "base64encodedimage1");
}

// Test ReplSession creation
TEST(ReplSessionTest, CreateWithClient) {
    auto client = std::make_shared<CliClient>("127.0.0.1", 11434);
    SessionSettings settings;
    settings.show_thinking = true;

    ReplSession session(client, "llama3.2:latest", settings);

    // Session should be created without errors
    EXPECT_EQ(session.getModelName(), "llama3.2:latest");
    EXPECT_TRUE(session.getSettings().show_thinking);
}

TEST(ReplSessionTest, GetModelName) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "mistral:7b");

    EXPECT_EQ(session.getModelName(), "mistral:7b");
}

TEST(ReplSessionTest, GetModelNameWithTag) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "qwen2.5:14b-q4_0");

    EXPECT_EQ(session.getModelName(), "qwen2.5:14b-q4_0");
}

TEST(ReplSessionTest, DefaultSettings) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "llama3.2");

    const auto& settings = session.getSettings();
    EXPECT_FLOAT_EQ(settings.temperature, 0.7f);
    EXPECT_FLOAT_EQ(settings.top_p, 0.9f);
    EXPECT_EQ(settings.max_tokens, 0);
    EXPECT_FALSE(settings.show_thinking);
    EXPECT_TRUE(settings.stream);
}

TEST(ReplSessionTest, CustomSettings) {
    auto client = std::make_shared<CliClient>();
    SessionSettings settings;
    settings.temperature = 0.3f;
    settings.max_tokens = 2048;
    settings.stream = false;

    ReplSession session(client, "llama3.2", settings);

    const auto& s = session.getSettings();
    EXPECT_FLOAT_EQ(s.temperature, 0.3f);
    EXPECT_EQ(s.max_tokens, 2048);
    EXPECT_FALSE(s.stream);
}

TEST(ReplSessionTest, UpdateSettings) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "llama3.2");

    SessionSettings new_settings;
    new_settings.temperature = 0.1f;
    new_settings.show_thinking = true;

    session.updateSettings(new_settings);

    const auto& s = session.getSettings();
    EXPECT_FLOAT_EQ(s.temperature, 0.1f);
    EXPECT_TRUE(s.show_thinking);
}

// Test history operations
TEST(ReplSessionTest, InitialHistoryEmpty) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "llama3.2");

    EXPECT_TRUE(session.getHistory().empty());
}

TEST(ReplSessionTest, ClearHistory) {
    auto client = std::make_shared<CliClient>();
    ReplSession session(client, "llama3.2");

    session.clearHistory();
    EXPECT_TRUE(session.getHistory().empty());
}

// Test MessageRole enum values
TEST(MessageRoleTest, EnumValues) {
    EXPECT_NE(static_cast<int>(MessageRole::System), static_cast<int>(MessageRole::User));
    EXPECT_NE(static_cast<int>(MessageRole::User), static_cast<int>(MessageRole::Assistant));
    EXPECT_NE(static_cast<int>(MessageRole::System), static_cast<int>(MessageRole::Assistant));
}
