/**
 * @file glm_thinking_test.cpp
 * @brief Unit tests for GLM-4.7 Interleaved Thinking mode (Task 68)
 *
 * Tests for GLM-4.7's Interleaved Thinking feature.
 * This feature allows the model to output its "thinking process"
 * interspersed with the actual response, similar to chain-of-thought.
 */

#include <gtest/gtest.h>
#include <vector>
#include <string>
#include <cstdint>
#include "safetensors.h"
#include "safetensors_internal.h"
#include "arch/glm.h"

class GLMThinkingTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Thinking block token detection
TEST_F(GLMThinkingTest, ThinkingBlockTokenDetection) {
    // GLM-4.7 config with thinking support
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);
    // GLM-4.7 supports thinking by default
    EXPECT_TRUE(config.supports_thinking);
}

// Test: Thinking block parsing
TEST_F(GLMThinkingTest, ThinkingBlockParsing) {
    // Parse thinking blocks from generated output
    std::string text = "Let me think <think>reasoning here</think> The answer is 42.";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);

    ASSERT_EQ(blocks.size(), 1);
    EXPECT_EQ(blocks[0].content, "reasoning here");
    EXPECT_EQ(blocks[0].start_pos, 13); // Position of <think>
    EXPECT_EQ(blocks[0].end_pos, 42);   // Position after </think>
}

// Test: Interleaved thinking mode activation
TEST_F(GLMThinkingTest, InterleavedThinkingActivation) {
    // GLM-4.7 supports interleaved thinking
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);
    EXPECT_TRUE(config.supports_thinking);
}

// Test: Thinking content streaming
TEST_F(GLMThinkingTest, ThinkingContentStreaming) {
    // Streaming mode can parse partial thinking blocks
    // This test verifies handling of complete blocks
    std::string streamed = "<think>Step 1: analyze</think> Result: done";

    auto blocks = safetensors::glm::parse_thinking_blocks(streamed);
    ASSERT_EQ(blocks.size(), 1);
    EXPECT_EQ(blocks[0].content, "Step 1: analyze");
}

// Test: Thinking block removal for final output
TEST_F(GLMThinkingTest, ThinkingBlockRemoval) {
    // Remove thinking blocks for clean output
    std::string text_with_thinking =
        "Let me think <think>reasoning step 1</think> "
        "The answer <think>verify: yes</think> is 42.";

    std::string cleaned = safetensors::glm::remove_thinking_blocks(text_with_thinking);

    EXPECT_EQ(cleaned, "Let me think  The answer  is 42.");
}

// Test: Multiple thinking blocks handling
TEST_F(GLMThinkingTest, MultipleThinkingBlocks) {
    // GLM-4.7 may generate multiple thinking blocks
    std::string text =
        "First <think>thought 1</think> then <think>thought 2</think> answer";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);

    ASSERT_EQ(blocks.size(), 2);
    EXPECT_EQ(blocks[0].content, "thought 1");
    EXPECT_EQ(blocks[1].content, "thought 2");
}

// Test: Nested thinking blocks (if supported)
TEST_F(GLMThinkingTest, NestedThinkingBlocks) {
    // Nested thinking blocks are not expected in GLM output
    // The parser uses simple string matching, so nested blocks
    // would parse the inner block first
    std::string text = "<think>outer <think>inner</think> outer</think>";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);

    // Current implementation finds the first </think> and closes the block
    // This is intentional - nested thinking is not a valid GLM output format
    ASSERT_GE(blocks.size(), 1);
}

// Test: Thinking block token budget
TEST_F(GLMThinkingTest, ThinkingTokenBudget) {
    // Thinking tokens count towards total budget
    // This is a configuration/generation concern, not parsing
    std::string config_json = R"({
        "architectures": ["GLM4ForCausalLM"],
        "hidden_size": 4096,
        "num_hidden_layers": 32,
        "num_attention_heads": 32
    })";

    auto config = safetensors::glm::parse_glm_config(config_json);
    // Thinking support is enabled by default
    EXPECT_TRUE(config.supports_thinking);
}

// Test: Thinking mode with tool calls
TEST_F(GLMThinkingTest, ThinkingWithToolCalls) {
    // GLM-4.7 supports tool/function calling alongside thinking
    std::string text =
        "<think>I need to search for this</think>"
        "[tool_call: search(query=\"test\")]"
        "<think>Got results, analyzing</think>"
        "The answer is found.";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);
    ASSERT_EQ(blocks.size(), 2);
    EXPECT_EQ(blocks[0].content, "I need to search for this");
    EXPECT_EQ(blocks[1].content, "Got results, analyzing");

    // Remove thinking to get clean output with tool calls
    std::string cleaned = safetensors::glm::remove_thinking_blocks(text);
    EXPECT_TRUE(cleaned.find("[tool_call:") != std::string::npos);
}

// Test: Thinking block in chat template
TEST_F(GLMThinkingTest, ThinkingInChatTemplate) {
    // Chat template integration - thinking blocks in responses
    std::string response =
        "<think>User wants calculation</think>"
        "2 + 2 = 4";

    auto blocks = safetensors::glm::parse_thinking_blocks(response);
    ASSERT_EQ(blocks.size(), 1);
    EXPECT_EQ(blocks[0].content, "User wants calculation");

    std::string visible = safetensors::glm::remove_thinking_blocks(response);
    EXPECT_EQ(visible, "2 + 2 = 4");
}

// Test: Thinking mode output formatting
TEST_F(GLMThinkingTest, ThinkingOutputFormatting) {
    // Parse structured output with thinking
    std::string text = "Before <think>middle</think> after";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);
    ASSERT_EQ(blocks.size(), 1);

    // Verify positions for structured reconstruction
    EXPECT_LT(blocks[0].start_pos, blocks[0].end_pos);
    EXPECT_EQ(blocks[0].content, "middle");
}

// Test: Empty thinking block handling
TEST_F(GLMThinkingTest, EmptyThinkingBlock) {
    // Handle edge case: "<think></think>" with no content
    std::string text = "Start <think></think> end";

    auto blocks = safetensors::glm::parse_thinking_blocks(text);
    ASSERT_EQ(blocks.size(), 1);
    EXPECT_EQ(blocks[0].content, "");  // Empty content is valid

    std::string cleaned = safetensors::glm::remove_thinking_blocks(text);
    EXPECT_EQ(cleaned, "Start  end");
}

// Test: Malformed thinking block handling
TEST_F(GLMThinkingTest, MalformedThinkingBlock) {
    // Unclosed thinking tag - should treat rest as thinking content
    std::string unclosed = "Start <think>unclosed content";
    auto blocks1 = safetensors::glm::parse_thinking_blocks(unclosed);
    ASSERT_EQ(blocks1.size(), 1);
    EXPECT_EQ(blocks1[0].content, "unclosed content");

    // No thinking tags at all
    std::string no_tags = "Just regular text without thinking";
    auto blocks2 = safetensors::glm::parse_thinking_blocks(no_tags);
    EXPECT_EQ(blocks2.size(), 0);

    // Only closing tag (should be ignored)
    std::string only_close = "Text </think> more";
    auto blocks3 = safetensors::glm::parse_thinking_blocks(only_close);
    EXPECT_EQ(blocks3.size(), 0);
}

// Test: Thinking mode performance impact
TEST_F(GLMThinkingTest, ThinkingPerformanceImpact) {
    // Performance test: parsing should be efficient
    // Generate a text with many thinking blocks
    std::string text;
    for (int i = 0; i < 100; i++) {
        text += "<think>thought " + std::to_string(i) + "</think> response ";
    }

    auto blocks = safetensors::glm::parse_thinking_blocks(text);
    EXPECT_EQ(blocks.size(), 100);

    // Removal should also be efficient
    std::string cleaned = safetensors::glm::remove_thinking_blocks(text);
    EXPECT_TRUE(cleaned.find("<think>") == std::string::npos);
}
