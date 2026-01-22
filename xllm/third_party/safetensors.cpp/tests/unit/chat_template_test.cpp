/**
 * @file chat_template_test.cpp
 * @brief Unit tests for chat template parsing and application (Task 20)
 */

#include <gtest/gtest.h>
#include "safetensors_internal.h"

class ChatTemplateTest : public ::testing::Test {
protected:
    // Test message structure
    struct Message {
        std::string role;
        std::string content;
    };
};

// Test: Simple template parsing
TEST_F(ChatTemplateTest, SimpleTemplateParsing) {
    const std::string template_str =
        "{% for message in messages %}"
        "{{ message.role }}: {{ message.content }}\n"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;

    bool result = stcpp::parse_chat_template(template_str, tmpl, error);

    EXPECT_TRUE(result) << "Error: " << error;
}

// Test: Apply template to messages
TEST_F(ChatTemplateTest, ApplyTemplateToMessages) {
    const std::string template_str =
        "{% for message in messages %}"
        "{{ message.role }}: {{ message.content }}\n"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"user", "Hello"},
        {"assistant", "Hi there!"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    EXPECT_NE(result.find("user"), std::string::npos);
    EXPECT_NE(result.find("Hello"), std::string::npos);
    EXPECT_NE(result.find("assistant"), std::string::npos);
    EXPECT_NE(result.find("Hi there!"), std::string::npos);
}

// Test: Chatml format template
TEST_F(ChatTemplateTest, ChatmlFormatTemplate) {
    const std::string template_str =
        "{% for message in messages %}"
        "<|im_start|>{{ message.role }}\n{{ message.content }}<|im_end|>\n"
        "{% endfor %}"
        "<|im_start|>assistant\n";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"system", "You are a helpful assistant."},
        {"user", "What is 2+2?"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    EXPECT_NE(result.find("<|im_start|>"), std::string::npos);
    EXPECT_NE(result.find("<|im_end|>"), std::string::npos);
    EXPECT_NE(result.find("system"), std::string::npos);
    EXPECT_NE(result.find("user"), std::string::npos);
}

// Test: Llama format template
TEST_F(ChatTemplateTest, LlamaFormatTemplate) {
    const std::string template_str =
        "{% for message in messages %}"
        "{% if message.role == 'system' %}"
        "<<SYS>>{{ message.content }}<</SYS>>\n\n"
        "{% elif message.role == 'user' %}"
        "[INST] {{ message.content }} [/INST]"
        "{% elif message.role == 'assistant' %}"
        "{{ message.content }}"
        "{% endif %}"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"system", "Be concise."},
        {"user", "Hello"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    EXPECT_NE(result.find("<<SYS>>"), std::string::npos);
    EXPECT_NE(result.find("[INST]"), std::string::npos);
}

// Test: Empty messages
TEST_F(ChatTemplateTest, EmptyMessages) {
    const std::string template_str =
        "{% for message in messages %}"
        "{{ message.content }}"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages;  // Empty

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success);
    EXPECT_EQ(result, "");
}

// Test: Invalid template syntax
TEST_F(ChatTemplateTest, InvalidTemplateSyntax) {
    const std::string template_str = "{% for message in messages %}{{ message.content }";  // Missing endfor

    stcpp::ChatTemplate tmpl;
    std::string error;

    bool result = stcpp::parse_chat_template(template_str, tmpl, error);

    EXPECT_FALSE(result);
    EXPECT_FALSE(error.empty());
}

// Test: Template with conditionals
TEST_F(ChatTemplateTest, TemplateWithConditionals) {
    const std::string template_str =
        "{% for message in messages %}"
        "{% if message.role == 'user' %}"
        "USER: {{ message.content }}\n"
        "{% else %}"
        "ASSISTANT: {{ message.content }}\n"
        "{% endif %}"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"user", "Question"},
        {"assistant", "Answer"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    EXPECT_NE(result.find("USER: Question"), std::string::npos);
    EXPECT_NE(result.find("ASSISTANT: Answer"), std::string::npos);
}

// Test: Template with special characters in content
TEST_F(ChatTemplateTest, SpecialCharactersInContent) {
    const std::string template_str =
        "{% for message in messages %}{{ message.content }}{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"user", "Hello <script>alert('xss')</script> world"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    // Content should be preserved (no HTML escaping needed for LLM)
    EXPECT_NE(result.find("<script>"), std::string::npos);
}

// Test: Template with loop index
TEST_F(ChatTemplateTest, TemplateWithLoopIndex) {
    const std::string template_str =
        "{% for message in messages %}"
        "[{{ loop.index }}] {{ message.content }}\n"
        "{% endfor %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"user", "First"},
        {"user", "Second"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error);

    EXPECT_TRUE(success) << "Error: " << error;
    // Note: loop.index in Jinja2 starts at 1
    EXPECT_NE(result.find("[1]"), std::string::npos);
    EXPECT_NE(result.find("[2]"), std::string::npos);
}

// Test: Generation prompt handling
TEST_F(ChatTemplateTest, GenerationPromptHandling) {
    const std::string template_str =
        "{% for message in messages %}"
        "{{ message.role }}: {{ message.content }}\n"
        "{% endfor %}"
        "{% if add_generation_prompt %}"
        "assistant:"
        "{% endif %}";

    stcpp::ChatTemplate tmpl;
    std::string error;
    ASSERT_TRUE(stcpp::parse_chat_template(template_str, tmpl, error));

    std::vector<stcpp::ChatMessage> messages = {
        {"user", "Hello"}
    };

    std::string result;
    bool success = stcpp::apply_chat_template(tmpl, messages, result, error, true);

    EXPECT_TRUE(success) << "Error: " << error;
    // Should end with "assistant:" when add_generation_prompt is true
    EXPECT_NE(result.find("assistant:"), std::string::npos);
}
