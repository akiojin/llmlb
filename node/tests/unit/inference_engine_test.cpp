#include <gtest/gtest.h>

#include "core/inference_engine.h"

using namespace llm_node;

extern "C" {
void llm_tokenizer_free_string(char* s);
bool llm_chat_template_render(
    const char* template_str,
    const char* messages_json,
    const char* special_tokens_json,
    bool add_generation_prompt,
    char** out_text,
    char** out_error);
}  // extern "C"

// テスト専用ヘルパー（inference_engine.cppで定義）
namespace llm_node {
std::string extractGptOssFinalMessageForTest(const std::string& output);
}
using llm_node::extractGptOssFinalMessageForTest;

TEST(InferenceEngineTest, GeneratesChatFromLastUserMessage) {
    InferenceEngine engine;
    std::vector<ChatMessage> msgs = {
        {"system", "You are a bot."},
        {"user", "Hello"},
        {"assistant", "Hi"},
        {"user", "How are you?"},
    };
    auto out = engine.generateChat(msgs, "dummy");
    EXPECT_NE(out.find("How are you?"), std::string::npos);
}

TEST(InferenceEngineTest, GeneratesCompletionFromPrompt) {
    InferenceEngine engine;
    auto out = engine.generateCompletion("Once upon a time", "dummy");
    EXPECT_NE(out.find("Once upon a time"), std::string::npos);
}

TEST(InferenceEngineTest, GeneratesTokensWithLimit) {
    InferenceEngine engine;
    auto tokens = engine.generateTokens("a b c d e f", 3);
    ASSERT_EQ(tokens.size(), 3u);
    EXPECT_EQ(tokens[0], "a");
    EXPECT_EQ(tokens[2], "c");
}

TEST(InferenceEngineTest, StreamsChatTokens) {
    InferenceEngine engine;
    std::vector<std::string> collected;
    std::vector<ChatMessage> msgs = {{"user", "hello stream test"}};
    auto tokens = engine.generateChatStream(msgs, 2, [&](const std::string& t) { collected.push_back(t); });
    ASSERT_EQ(tokens.size(), 2u);
    EXPECT_EQ(collected, tokens);
}

TEST(InferenceEngineTest, BatchGeneratesPerPrompt) {
    InferenceEngine engine;
    std::vector<std::string> prompts = {"one two", "alpha beta gamma"};
    auto outs = engine.generateBatch(prompts, 2);
    ASSERT_EQ(outs.size(), 2u);
    EXPECT_EQ(outs[0][0], "one");
    EXPECT_EQ(outs[1][1], "beta");
}

TEST(InferenceEngineTest, SampleNextTokenReturnsLast) {
    InferenceEngine engine;
    std::vector<std::string> tokens = {"x", "y", "z"};
    EXPECT_EQ(engine.sampleNextToken(tokens), "z");
}

TEST(InferenceEngineTest, ExtractsFinalChannelFromGptOssOutput) {
    const std::string raw =
        "<|start|>assistant<|channel|>analysis<|message|>think here<|end|>"
        "<|start|>assistant<|channel|>final<|message|>the answer<|end|>";

    auto extracted = extractGptOssFinalMessageForTest(raw);
    EXPECT_EQ(extracted, "the answer");
}

TEST(InferenceEngineTest, RendersChatTemplateWithMessages) {
    const char* tmpl = "{{ messages[0]['role'] }}: {{ messages[0]['content'] }}{% if add_generation_prompt %}<<GEN>>{% endif %}";
    const char* msgs = R"([{"role":"user","content":"hello"}])";
    const char* specials = R"({"bos_token":"","eos_token":""})";

    char* out = nullptr;
    char* err = nullptr;
    const bool ok = llm_chat_template_render(tmpl, msgs, specials, true, &out, &err);
    ASSERT_TRUE(ok) << (err ? err : "");
    ASSERT_NE(out, nullptr);

    const std::string rendered(out);
    llm_tokenizer_free_string(out);
    if (err) llm_tokenizer_free_string(err);

    EXPECT_NE(rendered.find("user: hello"), std::string::npos);
    EXPECT_NE(rendered.find("<<GEN>>"), std::string::npos);
}
