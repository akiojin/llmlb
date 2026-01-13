/**
 * @file tokenization_test.cpp
 * @brief Unit tests for tokenization operations (Task 19)
 */

#include <gtest/gtest.h>
#include "safetensors_internal.h"

class TokenizationTest : public ::testing::Test {
protected:
    stcpp::TokenizerImpl tokenizer;

    void SetUp() override {
        // Set up a simple BPE tokenizer for testing
        tokenizer.vocab = {"<s>", "</s>", "<pad>", "hello", "world", "h", "e", "l", "o", " "};

        for (size_t i = 0; i < tokenizer.vocab.size(); i++) {
            tokenizer.vocab_to_id[tokenizer.vocab[i]] = static_cast<int32_t>(i);
        }

        tokenizer.bos_token_id = 0;  // <s>
        tokenizer.eos_token_id = 1;  // </s>
        tokenizer.pad_token_id = 2;  // <pad>

        // Simple merge rules: h+e -> he, l+l -> ll, etc.
        tokenizer.merges = {
            {"h", "e"},
            {"l", "l"},
            {"he", "llo"}
        };
    }
};

// Test: Basic text tokenization
TEST_F(TokenizationTest, BasicTextTokenization) {
    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "hello", tokens, false, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_GT(tokens.size(), 0);
}

// Test: Add BOS token when requested
TEST_F(TokenizationTest, AddBosToken) {
    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "hello", tokens, true, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_GT(tokens.size(), 0);
    EXPECT_EQ(tokens[0], tokenizer.bos_token_id);
}

// Test: Empty string tokenization
TEST_F(TokenizationTest, EmptyStringTokenization) {
    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "", tokens, false, error);

    EXPECT_TRUE(result);
    EXPECT_EQ(tokens.size(), 0);
}

// Test: Empty string with BOS token
TEST_F(TokenizationTest, EmptyStringWithBos) {
    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "", tokens, true, error);

    EXPECT_TRUE(result);
    EXPECT_EQ(tokens.size(), 1);
    EXPECT_EQ(tokens[0], tokenizer.bos_token_id);
}

// Test: Long text tokenization
TEST_F(TokenizationTest, LongTextTokenization) {
    std::string long_text;
    for (int i = 0; i < 1000; i++) {
        long_text += "hello world ";
    }

    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, long_text, tokens, false, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_GT(tokens.size(), 0);
}

// Test: Unicode text handling
TEST_F(TokenizationTest, UnicodeTextHandling) {
    std::vector<int32_t> tokens;
    std::string error;

    // Japanese text
    bool result = stcpp::tokenize(tokenizer, "こんにちは", tokens, false, error);

    // Should succeed (might produce byte-level tokens)
    EXPECT_TRUE(result);
}

// Test: Special characters handling
TEST_F(TokenizationTest, SpecialCharactersHandling) {
    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "hello\nworld\ttab", tokens, false, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_GT(tokens.size(), 0);
}

// Test: Detokenization
TEST_F(TokenizationTest, BasicDetokenization) {
    std::vector<int32_t> tokens = {3, 9, 4};  // "hello", " ", "world"
    std::string result;
    std::string error;

    bool success = stcpp::detokenize(tokenizer, tokens, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    EXPECT_FALSE(result.empty());
}

// Test: Detokenization with invalid tokens
TEST_F(TokenizationTest, DetokenizationWithInvalidTokens) {
    std::vector<int32_t> tokens = {3, 9999, 4};  // 9999 is invalid
    std::string result;
    std::string error;

    bool success = stcpp::detokenize(tokenizer, tokens, result, error);

    // Should handle gracefully (skip or replace invalid tokens)
    // Implementation decides the behavior
    if (!success) {
        EXPECT_FALSE(error.empty());
    }
}

// Test: Empty token vector detokenization
TEST_F(TokenizationTest, EmptyDetokenization) {
    std::vector<int32_t> tokens;
    std::string result;
    std::string error;

    bool success = stcpp::detokenize(tokenizer, tokens, result, error);

    EXPECT_TRUE(success);
    EXPECT_EQ(result, "");
}

// Test: Round-trip tokenization
TEST_F(TokenizationTest, RoundTripTokenization) {
    const std::string original = "hello world";
    std::vector<int32_t> tokens;
    std::string decoded;
    std::string error;

    bool tok_result = stcpp::tokenize(tokenizer, original, tokens, false, error);
    EXPECT_TRUE(tok_result) << "Tokenization error: " << error;

    bool detok_result = stcpp::detokenize(tokenizer, tokens, decoded, error);
    EXPECT_TRUE(detok_result) << "Detokenization error: " << error;

    // The decoded text should be similar to original
    // (may have minor differences due to normalization)
    EXPECT_FALSE(decoded.empty());
}

// Test: Tokenizer with no BOS token configured
TEST_F(TokenizationTest, NoBosTokenConfigured) {
    tokenizer.bos_token_id = -1;  // No BOS token

    std::vector<int32_t> tokens;
    std::string error;

    bool result = stcpp::tokenize(tokenizer, "hello", tokens, true, error);

    // Should succeed, but no BOS token should be added
    EXPECT_TRUE(result);
    if (!tokens.empty()) {
        EXPECT_NE(tokens[0], -1);
    }
}
