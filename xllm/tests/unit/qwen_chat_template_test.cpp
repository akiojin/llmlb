/**
 * @file qwen_chat_template_test.cpp
 * @brief Qwen系モデルのチャットテンプレートテスト
 *
 * SPEC-6cd7f960: 検証済みモデル一覧
 * QwenモデルのChatMLフォーマットが正しく処理されることを検証する。
 */

#include <gtest/gtest.h>
#include "core/chat_template_renderer.h"

namespace {

class QwenChatTemplateTest : public ::testing::Test {
protected:
    // Qwen (ChatML) format template
    // Based on Qwen2.5/Qwen3 tokenizer_config.json
    const std::string qwen_template_ =
        "{% for message in messages %}"
        "{%- if message['role'] == 'system' -%}"
        "<|im_start|>system\n{{ message['content'] }}<|im_end|>\n"
        "{%- elif message['role'] == 'user' -%}"
        "<|im_start|>user\n{{ message['content'] }}<|im_end|>\n"
        "{%- elif message['role'] == 'assistant' -%}"
        "<|im_start|>assistant\n{{ message['content'] }}<|im_end|>\n"
        "{%- endif -%}"
        "{% endfor %}"
        "{% if add_generation_prompt %}"
        "<|im_start|>assistant\n"
        "{% endif %}";

    xllm::ChatTemplateRenderer renderer_ = xllm::ChatTemplateRenderer::fromString(
        qwen_template_,
        "",      // bos_token (Qwen uses empty or model-specific)
        "<|im_end|>"  // eos_token
    );
};

// T1: 基本的なユーザーメッセージのレンダリング
TEST_F(QwenChatTemplateTest, BasicUserMessage) {
    std::vector<xllm::ChatMessage> messages = {
        {"user", "Hello, how are you?"}
    };

    std::string result = renderer_.render(messages, true);

    // Should contain im_start/im_end markers
    EXPECT_NE(result.find("<|im_start|>user"), std::string::npos)
        << "Result: " << result;
    EXPECT_NE(result.find("Hello, how are you?"), std::string::npos);
    EXPECT_NE(result.find("<|im_end|>"), std::string::npos);

    // Should end with assistant prompt when add_generation_prompt is true
    EXPECT_NE(result.find("<|im_start|>assistant"), std::string::npos);
}

// T2: システムメッセージ + ユーザーメッセージ
TEST_F(QwenChatTemplateTest, SystemAndUserMessage) {
    std::vector<xllm::ChatMessage> messages = {
        {"system", "You are a helpful assistant."},
        {"user", "What is 2+2?"}
    };

    std::string result = renderer_.render(messages, true);

    // System message should come first
    size_t system_pos = result.find("<|im_start|>system");
    size_t user_pos = result.find("<|im_start|>user");

    EXPECT_NE(system_pos, std::string::npos);
    EXPECT_NE(user_pos, std::string::npos);
    EXPECT_LT(system_pos, user_pos) << "System message should come before user";

    // Check content
    EXPECT_NE(result.find("You are a helpful assistant."), std::string::npos);
    EXPECT_NE(result.find("What is 2+2?"), std::string::npos);
}

// T3: マルチターン会話
TEST_F(QwenChatTemplateTest, MultiTurnConversation) {
    std::vector<xllm::ChatMessage> messages = {
        {"system", "You are a math tutor."},
        {"user", "What is 2+2?"},
        {"assistant", "2+2 equals 4."},
        {"user", "What about 3+3?"}
    };

    std::string result = renderer_.render(messages, true);

    // Verify all messages appear in order
    size_t pos1 = result.find("You are a math tutor.");
    size_t pos2 = result.find("What is 2+2?");
    size_t pos3 = result.find("2+2 equals 4.");
    size_t pos4 = result.find("What about 3+3?");

    EXPECT_NE(pos1, std::string::npos);
    EXPECT_NE(pos2, std::string::npos);
    EXPECT_NE(pos3, std::string::npos);
    EXPECT_NE(pos4, std::string::npos);

    EXPECT_LT(pos1, pos2);
    EXPECT_LT(pos2, pos3);
    EXPECT_LT(pos3, pos4);

    // Count im_start occurrences (should be 5: system, user, assistant, user, + generation prompt)
    size_t count = 0;
    size_t pos = 0;
    while ((pos = result.find("<|im_start|>", pos)) != std::string::npos) {
        count++;
        pos++;
    }
    EXPECT_EQ(count, 5);
}

// T4: add_generation_prompt=falseの場合
TEST_F(QwenChatTemplateTest, NoGenerationPrompt) {
    std::vector<xllm::ChatMessage> messages = {
        {"user", "Hello"},
        {"assistant", "Hi there!"}
    };

    std::string result = renderer_.render(messages, false);

    // Should end with the last message's im_end, not with assistant prompt
    // Find last occurrence of <|im_end|>
    size_t last_im_end = result.rfind("<|im_end|>");
    EXPECT_NE(last_im_end, std::string::npos);

    // After the last im_end, there should be no <|im_start|>assistant
    std::string after_last = result.substr(last_im_end + 10);  // 10 = len("<|im_end|>")
    EXPECT_EQ(after_last.find("<|im_start|>assistant"), std::string::npos)
        << "Should not have assistant prompt when add_generation_prompt=false";
}

// T5: 日本語コンテンツのサポート
TEST_F(QwenChatTemplateTest, JapaneseContent) {
    std::vector<xllm::ChatMessage> messages = {
        {"system", "あなたは親切なアシスタントです。"},
        {"user", "こんにちは！元気ですか？"}
    };

    std::string result = renderer_.render(messages, true);

    EXPECT_NE(result.find("あなたは親切なアシスタントです。"), std::string::npos);
    EXPECT_NE(result.find("こんにちは！元気ですか？"), std::string::npos);
}

// T6: 特殊文字を含むコンテンツ
TEST_F(QwenChatTemplateTest, SpecialCharactersInContent) {
    std::vector<xllm::ChatMessage> messages = {
        {"user", "What does `print('hello')` do in Python?"}
    };

    std::string result = renderer_.render(messages, true);

    // Special characters should be preserved
    EXPECT_NE(result.find("`print('hello')`"), std::string::npos);
}

// T7: 空のメッセージリスト
TEST_F(QwenChatTemplateTest, EmptyMessageList) {
    std::vector<xllm::ChatMessage> messages;

    std::string result = renderer_.render(messages, true);

    // Should only have the generation prompt
    EXPECT_NE(result.find("<|im_start|>assistant"), std::string::npos);

    // Count should be 1 (only generation prompt)
    size_t count = 0;
    size_t pos = 0;
    while ((pos = result.find("<|im_start|>", pos)) != std::string::npos) {
        count++;
        pos++;
    }
    EXPECT_EQ(count, 1);
}

// T8: 長いコンテンツのサポート
TEST_F(QwenChatTemplateTest, LongContent) {
    // Create a long message (1000 chars)
    std::string long_content(1000, 'x');
    std::vector<xllm::ChatMessage> messages = {
        {"user", long_content}
    };

    std::string result = renderer_.render(messages, true);

    EXPECT_NE(result.find(long_content), std::string::npos);
}

// T9: EOSトークンの確認
TEST_F(QwenChatTemplateTest, EosTokenIsSet) {
    EXPECT_EQ(renderer_.eosToken(), "<|im_end|>");
}

}  // namespace
