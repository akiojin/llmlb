/**
 * @file tokenizer.cpp
 * @brief Tokenizer implementation with BPE support
 */

#include "safetensors_internal.h"
#include <fstream>
#include <filesystem>
#include <sstream>
#include <algorithm>
#include <regex>

namespace stcpp {

namespace json_parser {
extern void skip_ws(const char*& p, const char* end);
extern std::string parse_string(const char*& p, const char* end);
extern int64_t parse_int(const char*& p, const char* end);
extern void skip_value(const char*& p, const char* end);
}  // namespace json_parser

/* Parse vocab from JSON object */
static bool parse_vocab(
    const char*& p,
    const char* end,
    TokenizerImpl& tokenizer,
    std::string& error
) {
    json_parser::skip_ws(p, end);
    if (p >= end || *p != '{') {
        error = "Expected '{' for vocab";
        return false;
    }
    ++p;

    // Find max token ID to size vocab correctly
    int32_t max_id = -1;
    std::vector<std::pair<std::string, int32_t>> vocab_items;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == '}') break;

        std::string token = json_parser::parse_string(p, end);
        json_parser::skip_ws(p, end);
        if (p < end && *p == ':') ++p;
        json_parser::skip_ws(p, end);

        int32_t id = static_cast<int32_t>(json_parser::parse_int(p, end));
        vocab_items.push_back({token, id});
        if (id > max_id) max_id = id;

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }
    if (p < end && *p == '}') ++p;

    // Resize vocab and populate
    if (max_id >= 0) {
        tokenizer.vocab.resize(max_id + 1);
        for (const auto& item : vocab_items) {
            if (item.second >= 0 && item.second < static_cast<int32_t>(tokenizer.vocab.size())) {
                tokenizer.vocab[item.second] = item.first;
                tokenizer.vocab_to_id[item.first] = item.second;
            }
        }
    }

    return true;
}

/* Parse merge rules from JSON array */
static bool parse_merges(
    const char*& p,
    const char* end,
    TokenizerImpl& tokenizer,
    std::string& error
) {
    json_parser::skip_ws(p, end);
    if (p >= end || *p != '[') {
        error = "Expected '[' for merges";
        return false;
    }
    ++p;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == ']') break;

        std::string merge = json_parser::parse_string(p, end);

        // Parse "token1 token2" format
        size_t space = merge.find(' ');
        if (space != std::string::npos) {
            std::string first = merge.substr(0, space);
            std::string second = merge.substr(space + 1);
            tokenizer.merges.push_back({first, second});
        }

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }
    if (p < end && *p == ']') ++p;

    return true;
}

/* Parse added_tokens array for special tokens */
static bool parse_added_tokens(
    const char*& p,
    const char* end,
    TokenizerImpl& tokenizer,
    std::string& /*error*/
) {
    json_parser::skip_ws(p, end);
    if (p >= end || *p != '[') {
        return true;  // Optional field
    }
    ++p;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == ']') break;

        if (*p == '{') {
            ++p;
            int32_t id = -1;
            std::string content;
            bool is_special = false;

            while (p < end && *p != '}') {
                json_parser::skip_ws(p, end);
                std::string key = json_parser::parse_string(p, end);
                json_parser::skip_ws(p, end);
                if (p < end && *p == ':') ++p;
                json_parser::skip_ws(p, end);

                if (key == "id") {
                    id = static_cast<int32_t>(json_parser::parse_int(p, end));
                } else if (key == "content") {
                    content = json_parser::parse_string(p, end);
                } else if (key == "special") {
                    // Parse boolean
                    if (p + 4 <= end && strncmp(p, "true", 4) == 0) {
                        is_special = true;
                        p += 4;
                    } else if (p + 5 <= end && strncmp(p, "false", 5) == 0) {
                        is_special = false;
                        p += 5;
                    } else {
                        json_parser::skip_value(p, end);
                    }
                } else {
                    json_parser::skip_value(p, end);
                }

                json_parser::skip_ws(p, end);
                if (p < end && *p == ',') ++p;
            }
            if (p < end && *p == '}') ++p;

            // Identify special tokens by content
            if (is_special && id >= 0) {
                if (content == "<s>" || content == "<|begin_of_text|>" ||
                    content == "[CLS]" || content == "<bos>") {
                    tokenizer.bos_token_id = id;
                } else if (content == "</s>" || content == "<|end_of_text|>" ||
                           content == "[SEP]" || content == "<eos>") {
                    tokenizer.eos_token_id = id;
                } else if (content == "<pad>" || content == "[PAD]" ||
                           content == "<|pad|>") {
                    tokenizer.pad_token_id = id;
                }
            }
        } else {
            json_parser::skip_value(p, end);
        }

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }
    if (p < end && *p == ']') ++p;

    return true;
}

/* Load tokenizer from tokenizer.json */
bool load_tokenizer(
    const std::string& model_dir,
    TokenizerImpl& tokenizer,
    std::string& error
) {
    std::filesystem::path tokenizer_path = std::filesystem::path(model_dir) / "tokenizer.json";

    std::ifstream file(tokenizer_path);
    if (!file.is_open()) {
        error = "Failed to open tokenizer.json: " + tokenizer_path.string();
        return false;
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());

    const char* p = content.data();
    const char* end = p + content.size();

    json_parser::skip_ws(p, end);
    if (p >= end || *p != '{') {
        error = "Invalid tokenizer.json: expected '{'";
        return false;
    }
    ++p;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == '}') break;

        std::string key = json_parser::parse_string(p, end);
        json_parser::skip_ws(p, end);
        if (p < end && *p == ':') ++p;
        json_parser::skip_ws(p, end);

        if (key == "model") {
            // Parse model object containing vocab and merges
            if (p < end && *p == '{') {
                ++p;
                while (p < end && *p != '}') {
                    json_parser::skip_ws(p, end);
                    std::string model_key = json_parser::parse_string(p, end);
                    json_parser::skip_ws(p, end);
                    if (p < end && *p == ':') ++p;
                    json_parser::skip_ws(p, end);

                    if (model_key == "vocab") {
                        if (!parse_vocab(p, end, tokenizer, error)) {
                            return false;
                        }
                    } else if (model_key == "merges") {
                        if (!parse_merges(p, end, tokenizer, error)) {
                            return false;
                        }
                    } else {
                        json_parser::skip_value(p, end);
                    }

                    json_parser::skip_ws(p, end);
                    if (p < end && *p == ',') ++p;
                }
                if (p < end && *p == '}') ++p;
            }
        } else if (key == "added_tokens") {
            if (!parse_added_tokens(p, end, tokenizer, error)) {
                return false;
            }
        } else {
            json_parser::skip_value(p, end);
        }

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }

    // Load tokenizer_config.json for additional settings
    std::filesystem::path config_path = std::filesystem::path(model_dir) / "tokenizer_config.json";
    std::ifstream config_file(config_path);
    if (config_file.is_open()) {
        std::string config_content((std::istreambuf_iterator<char>(config_file)),
                                    std::istreambuf_iterator<char>());

        const char* cp = config_content.data();
        const char* cend = cp + config_content.size();

        json_parser::skip_ws(cp, cend);
        if (cp < cend && *cp == '{') {
            ++cp;
            while (cp < cend) {
                json_parser::skip_ws(cp, cend);
                if (cp >= cend || *cp == '}') break;

                std::string cfg_key = json_parser::parse_string(cp, cend);
                json_parser::skip_ws(cp, cend);
                if (cp < cend && *cp == ':') ++cp;
                json_parser::skip_ws(cp, cend);

                if (cfg_key == "bos_token") {
                    std::string token = json_parser::parse_string(cp, cend);
                    auto it = tokenizer.vocab_to_id.find(token);
                    if (it != tokenizer.vocab_to_id.end()) {
                        tokenizer.bos_token_id = it->second;
                    }
                } else if (cfg_key == "eos_token") {
                    std::string token = json_parser::parse_string(cp, cend);
                    auto it = tokenizer.vocab_to_id.find(token);
                    if (it != tokenizer.vocab_to_id.end()) {
                        tokenizer.eos_token_id = it->second;
                    }
                } else if (cfg_key == "pad_token") {
                    std::string token = json_parser::parse_string(cp, cend);
                    auto it = tokenizer.vocab_to_id.find(token);
                    if (it != tokenizer.vocab_to_id.end()) {
                        tokenizer.pad_token_id = it->second;
                    }
                } else if (cfg_key == "chat_template") {
                    tokenizer.chat_template = json_parser::parse_string(cp, cend);
                } else {
                    json_parser::skip_value(cp, cend);
                }

                json_parser::skip_ws(cp, cend);
                if (cp < cend && *cp == ',') ++cp;
            }
        }
    }

    return true;
}

/* Simple BPE tokenization */
bool tokenize(
    const TokenizerImpl& tokenizer,
    const std::string& text,
    std::vector<int32_t>& tokens,
    bool add_bos,
    std::string& error
) {
    tokens.clear();

    // Add BOS token if requested
    if (add_bos && tokenizer.bos_token_id >= 0) {
        tokens.push_back(tokenizer.bos_token_id);
    }

    if (text.empty()) {
        return true;
    }

    // Simple character-level fallback with BPE merge
    // In a full implementation, this would use proper BPE algorithm

    // First, split text into characters (UTF-8 aware)
    std::vector<std::string> chars;
    size_t i = 0;
    while (i < text.size()) {
        size_t char_len = 1;
        unsigned char c = text[i];
        if ((c & 0xE0) == 0xC0) char_len = 2;
        else if ((c & 0xF0) == 0xE0) char_len = 3;
        else if ((c & 0xF8) == 0xF0) char_len = 4;

        if (i + char_len <= text.size()) {
            chars.push_back(text.substr(i, char_len));
        }
        i += char_len;
    }

    // Apply BPE merges iteratively
    for (const auto& merge : tokenizer.merges) {
        std::vector<std::string> new_chars;
        size_t j = 0;
        while (j < chars.size()) {
            if (j + 1 < chars.size() &&
                chars[j] == merge.first &&
                chars[j + 1] == merge.second) {
                new_chars.push_back(chars[j] + chars[j + 1]);
                j += 2;
            } else {
                new_chars.push_back(chars[j]);
                j++;
            }
        }
        chars = std::move(new_chars);
    }

    // Look up token IDs
    for (const auto& tok : chars) {
        auto it = tokenizer.vocab_to_id.find(tok);
        if (it != tokenizer.vocab_to_id.end()) {
            tokens.push_back(it->second);
        } else {
            // Try byte fallback (common in modern tokenizers)
            for (unsigned char byte : tok) {
                std::string byte_token = "<0x" +
                    std::string(1, "0123456789ABCDEF"[byte >> 4]) +
                    std::string(1, "0123456789ABCDEF"[byte & 0xF]) + ">";
                auto byte_it = tokenizer.vocab_to_id.find(byte_token);
                if (byte_it != tokenizer.vocab_to_id.end()) {
                    tokens.push_back(byte_it->second);
                }
                // If byte fallback also fails, skip (could add UNK token)
            }
        }
    }

    return true;
}

/* Detokenize token IDs to text */
bool detokenize(
    const TokenizerImpl& tokenizer,
    const std::vector<int32_t>& tokens,
    std::string& result,
    std::string& error
) {
    result.clear();

    for (int32_t id : tokens) {
        if (id < 0) {
            continue;  // Skip invalid tokens
        }
        if (id >= static_cast<int32_t>(tokenizer.vocab.size())) {
            // Invalid token ID - skip or error
            continue;
        }

        const std::string& token = tokenizer.vocab[id];

        // Skip special tokens in output
        if (id == tokenizer.bos_token_id ||
            id == tokenizer.eos_token_id ||
            id == tokenizer.pad_token_id) {
            continue;
        }

        // Handle byte tokens like <0xXX>
        if (token.size() == 6 && token.substr(0, 3) == "<0x" && token[5] == '>') {
            char hex[3] = {token[3], token[4], 0};
            unsigned int byte_val;
            if (sscanf(hex, "%x", &byte_val) == 1) {
                result += static_cast<char>(byte_val);
                continue;
            }
        }

        result += token;
    }

    return true;
}

}  // namespace stcpp
