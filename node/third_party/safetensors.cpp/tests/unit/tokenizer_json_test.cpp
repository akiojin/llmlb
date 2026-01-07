/**
 * @file tokenizer_json_test.cpp
 * @brief Unit tests for tokenizer.json parsing (Task 18)
 */

#include <gtest/gtest.h>
#include <fstream>
#include <filesystem>
#include "safetensors_internal.h"

namespace fs = std::filesystem;

class TokenizerJsonTest : public ::testing::Test {
protected:
    fs::path temp_dir;

    void SetUp() override {
        temp_dir = fs::temp_directory_path() / "stcpp_tokenizer_test";
        fs::create_directories(temp_dir);
    }

    void TearDown() override {
        fs::remove_all(temp_dir);
    }

    void CreateTokenizerJson(const fs::path& dir, const std::string& content) {
        std::ofstream file(dir / "tokenizer.json");
        file << content;
        file.close();
    }
};

// Test: Load vocab from tokenizer.json
TEST_F(TokenizerJsonTest, LoadVocabFromJson) {
    const std::string json = R"({
        "model": {
            "type": "BPE",
            "vocab": {
                "hello": 0,
                "world": 1,
                "<s>": 2,
                "</s>": 3
            },
            "merges": []
        },
        "added_tokens": []
    })";
    CreateTokenizerJson(temp_dir, json);

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(tokenizer.vocab.size(), 4);
    EXPECT_EQ(tokenizer.vocab_to_id["hello"], 0);
    EXPECT_EQ(tokenizer.vocab_to_id["world"], 1);
}

// Test: Load BPE merge rules
TEST_F(TokenizerJsonTest, LoadBPEMergeRules) {
    const std::string json = R"({
        "model": {
            "type": "BPE",
            "vocab": {
                "h": 0,
                "e": 1,
                "l": 2,
                "o": 3,
                "he": 4,
                "ll": 5,
                "hello": 6
            },
            "merges": [
                "h e",
                "l l",
                "he llo"
            ]
        },
        "added_tokens": []
    })";
    CreateTokenizerJson(temp_dir, json);

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(tokenizer.merges.size(), 3);
    EXPECT_EQ(tokenizer.merges[0].first, "h");
    EXPECT_EQ(tokenizer.merges[0].second, "e");
    EXPECT_EQ(tokenizer.merges[1].first, "l");
    EXPECT_EQ(tokenizer.merges[1].second, "l");
}

// Test: Load special tokens
TEST_F(TokenizerJsonTest, LoadSpecialTokens) {
    const std::string json = R"({
        "model": {
            "type": "BPE",
            "vocab": {
                "<s>": 0,
                "</s>": 1,
                "<pad>": 2,
                "hello": 3
            },
            "merges": []
        },
        "added_tokens": [
            {"id": 0, "content": "<s>", "special": true},
            {"id": 1, "content": "</s>", "special": true},
            {"id": 2, "content": "<pad>", "special": true}
        ]
    })";
    CreateTokenizerJson(temp_dir, json);

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(tokenizer.bos_token_id, 0);
    EXPECT_EQ(tokenizer.eos_token_id, 1);
    EXPECT_EQ(tokenizer.pad_token_id, 2);
}

// Test: Load tokenizer_config.json for special token IDs
TEST_F(TokenizerJsonTest, LoadSpecialTokensFromConfig) {
    const std::string tokenizer_json = R"({
        "model": {
            "type": "BPE",
            "vocab": {"<s>": 1, "</s>": 2, "<pad>": 0},
            "merges": []
        },
        "added_tokens": []
    })";
    CreateTokenizerJson(temp_dir, tokenizer_json);

    std::ofstream config(temp_dir / "tokenizer_config.json");
    config << R"({
        "bos_token": "<s>",
        "eos_token": "</s>",
        "pad_token": "<pad>"
    })";
    config.close();

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_EQ(tokenizer.bos_token_id, 1);
    EXPECT_EQ(tokenizer.eos_token_id, 2);
    EXPECT_EQ(tokenizer.pad_token_id, 0);
}

// Test: Missing tokenizer.json returns error
TEST_F(TokenizerJsonTest, MissingTokenizerJsonReturnsError) {
    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_FALSE(result);
    EXPECT_FALSE(error.empty());
}

// Test: Invalid JSON returns error
TEST_F(TokenizerJsonTest, InvalidJsonReturnsError) {
    CreateTokenizerJson(temp_dir, "{ invalid json }");

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_FALSE(result);
    EXPECT_FALSE(error.empty());
}

// Test: Empty vocab handled gracefully
TEST_F(TokenizerJsonTest, EmptyVocabHandled) {
    const std::string json = R"({
        "model": {
            "type": "BPE",
            "vocab": {},
            "merges": []
        },
        "added_tokens": []
    })";
    CreateTokenizerJson(temp_dir, json);

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    // Should succeed but vocab will be empty
    EXPECT_TRUE(result);
    EXPECT_EQ(tokenizer.vocab.size(), 0);
}

// Test: Load chat template from tokenizer_config.json
TEST_F(TokenizerJsonTest, LoadChatTemplate) {
    const std::string tokenizer_json = R"({
        "model": {"type": "BPE", "vocab": {}, "merges": []},
        "added_tokens": []
    })";
    CreateTokenizerJson(temp_dir, tokenizer_json);

    std::ofstream config(temp_dir / "tokenizer_config.json");
    config << R"({
        "chat_template": "{% for message in messages %}{{ message.content }}{% endfor %}"
    })";
    config.close();

    stcpp::TokenizerImpl tokenizer;
    std::string error;
    bool result = stcpp::load_tokenizer(temp_dir.string(), tokenizer, error);

    EXPECT_TRUE(result) << "Error: " << error;
    EXPECT_FALSE(tokenizer.chat_template.empty());
    EXPECT_NE(tokenizer.chat_template.find("message"), std::string::npos);
}
