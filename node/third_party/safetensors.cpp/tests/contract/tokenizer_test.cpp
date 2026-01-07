/**
 * @file tokenizer_test.cpp
 * @brief Contract tests for tokenizer operations
 */

#include <gtest/gtest.h>
#include "safetensors.h"
#include <vector>

class TokenizerTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Tokenize with nullptr tokenizer returns 0
TEST_F(TokenizerTest, TokenizeNullTokenizerReturnsZero) {
    int32_t tokens[100];
    int32_t count = stcpp_tokenize(
        nullptr,
        "Hello, world!",
        tokens,
        100,
        false
    );
    EXPECT_EQ(count, 0);
}

// Test: Tokenize with nullptr text returns 0
TEST_F(TokenizerTest, TokenizeNullTextReturnsZero) {
    int32_t tokens[100];
    int32_t count = stcpp_tokenize(
        nullptr,  // No tokenizer available in stub
        nullptr,
        tokens,
        100,
        false
    );
    EXPECT_EQ(count, 0);
}

// Test: Tokenize with nullptr output buffer returns 0
TEST_F(TokenizerTest, TokenizeNullBufferReturnsZero) {
    int32_t count = stcpp_tokenize(
        nullptr,
        "Hello",
        nullptr,
        0,
        false
    );
    EXPECT_EQ(count, 0);
}

// Test: Tokenize with zero max_tokens returns 0
TEST_F(TokenizerTest, TokenizeZeroMaxTokensReturnsZero) {
    int32_t tokens[100];
    int32_t count = stcpp_tokenize(
        nullptr,
        "Hello",
        tokens,
        0,
        false
    );
    EXPECT_EQ(count, 0);
}

// Test: Detokenize with nullptr tokenizer returns 0
TEST_F(TokenizerTest, DetokenizeNullTokenizerReturnsZero) {
    int32_t tokens[] = {1, 2, 3};
    char text[100];
    int32_t length = stcpp_detokenize(
        nullptr,
        tokens,
        3,
        text,
        100
    );
    EXPECT_EQ(length, 0);
}

// Test: Detokenize with nullptr tokens returns 0
TEST_F(TokenizerTest, DetokenizeNullTokensReturnsZero) {
    char text[100];
    int32_t length = stcpp_detokenize(
        nullptr,
        nullptr,
        0,
        text,
        100
    );
    EXPECT_EQ(length, 0);
}

// Test: Detokenize with nullptr output buffer returns 0
TEST_F(TokenizerTest, DetokenizeNullBufferReturnsZero) {
    int32_t tokens[] = {1, 2, 3};
    int32_t length = stcpp_detokenize(
        nullptr,
        tokens,
        3,
        nullptr,
        0
    );
    EXPECT_EQ(length, 0);
}

// Test: Apply chat template with nullptr tokenizer returns 0
TEST_F(TokenizerTest, ApplyChatTemplateNullTokenizerReturnsZero) {
    char output[1000];
    int32_t length = stcpp_apply_chat_template(
        nullptr,
        R"([{"role":"user","content":"Hello"}])",
        output,
        1000,
        true
    );
    EXPECT_EQ(length, 0);
}

// Test: Apply chat template with nullptr messages returns 0
TEST_F(TokenizerTest, ApplyChatTemplateNullMessagesReturnsZero) {
    char output[1000];
    int32_t length = stcpp_apply_chat_template(
        nullptr,
        nullptr,
        output,
        1000,
        true
    );
    EXPECT_EQ(length, 0);
}

// Test: Apply chat template with nullptr output returns 0
TEST_F(TokenizerTest, ApplyChatTemplateNullOutputReturnsZero) {
    int32_t length = stcpp_apply_chat_template(
        nullptr,
        R"([{"role":"user","content":"Hello"}])",
        nullptr,
        0,
        true
    );
    EXPECT_EQ(length, 0);
}

// Test: BOS token with nullptr tokenizer returns -1
TEST_F(TokenizerTest, BosTokenNullTokenizerReturnsNegative) {
    int32_t bos = stcpp_token_bos(nullptr);
    EXPECT_EQ(bos, -1);
}

// Test: EOS token with nullptr tokenizer returns -1
TEST_F(TokenizerTest, EosTokenNullTokenizerReturnsNegative) {
    int32_t eos = stcpp_token_eos(nullptr);
    EXPECT_EQ(eos, -1);
}

// Test: PAD token with nullptr tokenizer returns -1
TEST_F(TokenizerTest, PadTokenNullTokenizerReturnsNegative) {
    int32_t pad = stcpp_token_pad(nullptr);
    EXPECT_EQ(pad, -1);
}
