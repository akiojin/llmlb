/**
 * @file tokenizer.cpp
 * @brief Tokenizer implementation with BPE support
 */

#include "safetensors_internal.h"
#include <fstream>
#include <filesystem>
#include <sstream>
#include <algorithm>
#include <cstring>
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

            // Add all added_tokens to vocab (critical for special token tokenization)
            if (id >= 0 && !content.empty()) {
                // Ensure vocab is large enough
                if (id >= static_cast<int32_t>(tokenizer.vocab.size())) {
                    tokenizer.vocab.resize(id + 1);
                }
                tokenizer.vocab[id] = content;
                tokenizer.vocab_to_id[content] = id;

                // Mark special tokens
                if (is_special) {
                    tokenizer.special_tokens.insert(content);
                    fprintf(stderr, "[DEBUG] Special token added: '%s' id=%d\n", content.c_str(), id);
                    fflush(stderr);
                }

                // Identify bos/eos/pad tokens by content
                if (is_special) {
                    if (content == "<s>" || content == "<|begin_of_text|>" ||
                        content == "[CLS]" || content == "<bos>" || content == "<|im_start|>") {
                        tokenizer.bos_token_id = id;
                    } else if (content == "</s>" || content == "<|end_of_text|>" ||
                               content == "[SEP]" || content == "<eos>" ||
                               content == "<|endoftext|>" || content == "<|im_end|>") {
                        tokenizer.eos_token_id = id;
                    } else if (content == "<pad>" || content == "[PAD]" ||
                               content == "<|pad|>") {
                        tokenizer.pad_token_id = id;
                    }
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

    fprintf(stderr, "[DEBUG] load_tokenizer: vocab loaded, vocab_size=%zu, merges=%zu\n",
            tokenizer.vocab.size(), tokenizer.merges.size());
    fflush(stderr);

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

    // Debug: Print tokenizer info
    fprintf(stderr, "[DEBUG] load_tokenizer: vocab_size=%zu, bos_id=%d, eos_id=%d, pad_id=%d\n",
            tokenizer.vocab.size(), tokenizer.bos_token_id, tokenizer.eos_token_id, tokenizer.pad_token_id);
    fflush(stderr);

    return true;
}

/* GPT-2 byte encoder table - maps bytes to Unicode characters */
static std::string byte_to_unicode(unsigned char b) {
    // GPT-2 uses a specific mapping for bytes to Unicode
    // Printable ASCII characters (except space) map to themselves
    // Other bytes map to Unicode characters starting at U+0100
    if (b >= 33 && b <= 126) {
        // Printable ASCII (! to ~)
        return std::string(1, static_cast<char>(b));
    }
    // Map other bytes to Unicode code points
    // The GPT-2 encoding uses:
    // 0x00-0x20 -> U+0100-U+0120
    // 0x7F-0xFF -> after that
    int codepoint;
    if (b <= 32) {
        codepoint = 0x0100 + b;  // 0x00->U+0100, 0x20(space)->U+0120='Ġ'
    } else if (b == 127) {
        codepoint = 0x0100 + 33;  // DEL
    } else {
        // 0x80-0xFF -> continue sequence
        codepoint = 0x0100 + 34 + (b - 128);  // 0x80->U+0122, etc.
    }
    // Encode as UTF-8
    std::string result;
    if (codepoint <= 0x7F) {
        result += static_cast<char>(codepoint);
    } else if (codepoint <= 0x7FF) {
        result += static_cast<char>(0xC0 | (codepoint >> 6));
        result += static_cast<char>(0x80 | (codepoint & 0x3F));
    }
    return result;
}

/* Convert text to GPT-2 byte-level encoding */
static std::string text_to_gpt2_bytes(const std::string& text) {
    std::string result;
    for (unsigned char c : text) {
        result += byte_to_unicode(c);
    }
    return result;
}

/* Reverse mapping: Unicode codepoint to original byte */
static int unicode_to_byte(int codepoint) {
    // Reverse the byte_to_unicode mapping
    // Printable ASCII (33-126) map to themselves
    if (codepoint >= 33 && codepoint <= 126) {
        return codepoint;
    }
    // U+0100-U+0120 -> bytes 0x00-0x20 (33 values)
    if (codepoint >= 0x0100 && codepoint <= 0x0120) {
        return codepoint - 0x0100;
    }
    // U+0121 -> DEL (127)
    if (codepoint == 0x0121) {
        return 127;
    }
    // U+0122-U+01A1 -> bytes 0x80-0xFF (128 values: 128-255)
    // byte = 128 + (codepoint - 0x0122)
    // max byte = 128 + (0x01A1 - 0x0122) = 128 + 127 = 255
    if (codepoint >= 0x0122 && codepoint <= 0x01A1) {
        return 128 + (codepoint - 0x0122);
    }
    // Not a GPT-2 byte-encoded character, return as-is if ASCII
    if (codepoint < 128) {
        return codepoint;
    }
    return -1;  // Invalid
}

/* Convert GPT-2 byte-level encoding back to original text (hybrid mode)
 * Only converts GPT-2 specific codepoints (U+0100-U+01A1), preserves other UTF-8 as-is.
 * This handles hybrid tokenizers like Qwen2 that mix GPT-2 encoding with regular UTF-8.
 */
static std::string gpt2_bytes_to_text(const std::string& encoded) {
    std::string result;
    const unsigned char* p = reinterpret_cast<const unsigned char*>(encoded.data());
    const unsigned char* end = p + encoded.size();

    while (p < end) {
        int codepoint;
        const unsigned char* char_start = p;
        size_t char_len = 1;

        // Decode UTF-8 to get codepoint
        if ((*p & 0x80) == 0) {
            // ASCII
            codepoint = *p++;
        } else if ((*p & 0xE0) == 0xC0 && p + 1 < end) {
            // 2-byte UTF-8
            codepoint = ((*p & 0x1F) << 6) | (*(p + 1) & 0x3F);
            char_len = 2;
            p += 2;
        } else if ((*p & 0xF0) == 0xE0 && p + 2 < end) {
            // 3-byte UTF-8
            codepoint = ((*p & 0x0F) << 12) | ((*(p + 1) & 0x3F) << 6) | (*(p + 2) & 0x3F);
            char_len = 3;
            p += 3;
        } else if ((*p & 0xF8) == 0xF0 && p + 3 < end) {
            // 4-byte UTF-8
            codepoint = ((*p & 0x07) << 18) | ((*(p + 1) & 0x3F) << 12) |
                        ((*(p + 2) & 0x3F) << 6) | (*(p + 3) & 0x3F);
            char_len = 4;
            p += 4;
        } else {
            // Invalid UTF-8, skip byte
            ++p;
            continue;
        }

        // Check if this is a GPT-2 encoded codepoint (U+0100-U+01A1)
        // These are the special byte-encoding characters used by GPT-2
        if (codepoint >= 0x0100 && codepoint <= 0x01A1) {
            // Convert GPT-2 encoded codepoint to original byte
            int byte_val = unicode_to_byte(codepoint);
            if (byte_val >= 0 && byte_val <= 255) {
                result += static_cast<char>(byte_val);
            }
        } else {
            // Not a GPT-2 encoded character - preserve as-is (e.g., Chinese, emoji)
            result.append(reinterpret_cast<const char*>(char_start), char_len);
        }
    }

    return result;
}

/* BPE tokenize a segment (no special tokens) */
static void bpe_tokenize_segment(
    const TokenizerImpl& tokenizer,
    const std::string& segment,
    std::vector<int32_t>& tokens
) {
    if (segment.empty()) return;

    // Convert text to GPT-2 byte encoding
    std::string encoded = text_to_gpt2_bytes(segment);

    // Split into individual byte-encoded characters (UTF-8 aware)
    std::vector<std::string> chars;
    size_t i = 0;
    while (i < encoded.size()) {
        size_t char_len = 1;
        unsigned char c = encoded[i];
        if ((c & 0xE0) == 0xC0) char_len = 2;
        else if ((c & 0xF0) == 0xE0) char_len = 3;
        else if ((c & 0xF8) == 0xF0) char_len = 4;

        if (i + char_len <= encoded.size()) {
            chars.push_back(encoded.substr(i, char_len));
        }
        i += char_len;
    }

    fprintf(stderr, "[DEBUG] tokenize: split into %zu chars\n", chars.size());
    fflush(stderr);

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

    fprintf(stderr, "[DEBUG] tokenize: after BPE, %zu pieces\n", chars.size());
    fflush(stderr);

    // Look up token IDs
    int found = 0, not_found = 0, byte_fallback = 0;
    for (const auto& tok : chars) {
        auto it = tokenizer.vocab_to_id.find(tok);
        if (it != tokenizer.vocab_to_id.end()) {
            tokens.push_back(it->second);
            found++;
        } else {
            // Try byte fallback
            for (unsigned char byte : tok) {
                std::string byte_token = "<0x" +
                    std::string(1, "0123456789ABCDEF"[byte >> 4]) +
                    std::string(1, "0123456789ABCDEF"[byte & 0xF]) + ">";
                auto byte_it = tokenizer.vocab_to_id.find(byte_token);
                if (byte_it != tokenizer.vocab_to_id.end()) {
                    tokens.push_back(byte_it->second);
                    byte_fallback++;
                }
            }
        }
    }
}

/* Simple BPE tokenization with special token handling */
bool tokenize(
    const TokenizerImpl& tokenizer,
    const std::string& text,
    std::vector<int32_t>& tokens,
    bool add_bos,
    std::string& error
) {
    fprintf(stderr, "[DEBUG] tokenize: entered, text_len=%zu, vocab_size=%zu, merges=%zu, special_tokens=%zu\n",
            text.size(), tokenizer.vocab.size(), tokenizer.merges.size(), tokenizer.special_tokens.size());
    fflush(stderr);

    tokens.clear();

    // Add BOS token if requested
    if (add_bos && tokenizer.bos_token_id >= 0) {
        tokens.push_back(tokenizer.bos_token_id);
        fprintf(stderr, "[DEBUG] tokenize: added BOS token %d\n", tokenizer.bos_token_id);
        fflush(stderr);
    }

    if (text.empty()) {
        fprintf(stderr, "[DEBUG] tokenize: empty text, returning\n");
        fflush(stderr);
        return true;
    }

    // Process text, extracting special tokens first
    size_t pos = 0;
    while (pos < text.size()) {
        bool found_special = false;

        // Try to match special tokens at current position
        for (const auto& special : tokenizer.special_tokens) {
            if (pos + special.size() <= text.size() &&
                text.compare(pos, special.size(), special) == 0) {
                // Found a special token
                auto it = tokenizer.vocab_to_id.find(special);
                if (it != tokenizer.vocab_to_id.end()) {
                    tokens.push_back(it->second);
                    fprintf(stderr, "[DEBUG] tokenize: found special token '%s' -> %d\n",
                            special.c_str(), it->second);
                    fflush(stderr);
                }
                pos += special.size();
                found_special = true;
                break;
            }
        }

        if (!found_special) {
            // Find the next special token position (or end of text)
            size_t next_special_pos = text.size();
            for (const auto& special : tokenizer.special_tokens) {
                size_t sp_pos = text.find(special, pos);
                if (sp_pos != std::string::npos && sp_pos < next_special_pos) {
                    next_special_pos = sp_pos;
                }
            }

            // Extract segment between current position and next special token
            if (next_special_pos > pos) {
                std::string segment = text.substr(pos, next_special_pos - pos);
                bpe_tokenize_segment(tokenizer, segment, tokens);
                pos = next_special_pos;
            }
        }
    }

    fprintf(stderr, "[DEBUG] tokenize: final_tokens=%zu\n", tokens.size());
    if (tokens.size() > 0) {
        fprintf(stderr, "[DEBUG] tokenize: first 5 tokens: ");
        for (size_t i = 0; i < std::min(tokens.size(), (size_t)5); i++) {
            fprintf(stderr, "%d ", tokens[i]);
        }
        fprintf(stderr, "\n");
    }
    fflush(stderr);

    return true;
}

/* Detect if tokenizer uses GPT-2 byte-level encoding */
static bool uses_gpt2_byte_encoding(const TokenizerImpl& tokenizer) {
    // Check for GPT-2 style tokens like "Ġ" (U+0120 = space in GPT-2 encoding)
    // Qwen2/LLama3 use different encoding where tokens are plain UTF-8
    //
    // GPT-2 encoding: " hello" is stored as "Ġhello" where Ġ = U+0120
    // Qwen2 encoding: tokens are stored as-is in UTF-8
    //
    // Detection: if vocab contains tokens starting with U+0100-U+01FF range
    // (GPT-2 byte encoding), we need GPT-2 decoding. Otherwise, use direct UTF-8.

    // Check first 1000 tokens for GPT-2 encoding markers
    size_t gpt2_marker_count = 0;
    size_t checked = 0;
    for (size_t i = 0; i < std::min(tokenizer.vocab.size(), (size_t)1000); i++) {
        const std::string& token = tokenizer.vocab[i];
        if (token.empty()) continue;
        checked++;

        // Check for 2-byte UTF-8 sequences in U+0100-U+01FF range
        // These are encoded as 0xC4 0x80-0xBF (U+0100-U+013F) or 0xC5 0x80-0xBF (U+0140-U+017F)
        // or 0xC6 0x80-0xBF (U+0180-U+01BF) or 0xC7 0x80-0x81 (U+01C0-U+01C1, but we go to U+01FF)
        for (size_t j = 0; j + 1 < token.size(); j++) {
            unsigned char c1 = static_cast<unsigned char>(token[j]);
            unsigned char c2 = static_cast<unsigned char>(token[j + 1]);
            // Check for C4/C5/C6/C7 prefix (covers U+0100-U+01FF)
            if ((c1 == 0xC4 || c1 == 0xC5 || c1 == 0xC6 || c1 == 0xC7) &&
                (c2 & 0xC0) == 0x80) {
                gpt2_marker_count++;
                break;  // Count once per token
            }
        }
    }

    // If more than 10% of checked tokens have GPT-2 markers, assume GPT-2 encoding
    bool uses_gpt2 = (checked > 0) && (gpt2_marker_count * 10 > checked);
    fprintf(stderr, "[DEBUG] uses_gpt2_byte_encoding: checked=%zu, gpt2_markers=%zu, uses_gpt2=%d\n",
            checked, gpt2_marker_count, uses_gpt2);
    fflush(stderr);
    return uses_gpt2;
}

/* Detokenize token IDs to text */
bool detokenize(
    const TokenizerImpl& tokenizer,
    const std::vector<int32_t>& tokens,
    std::string& result,
    std::string& error
) {
    result.clear();
    std::string accumulated;

    fprintf(stderr, "[DEBUG] detokenize: %zu tokens, vocab_size=%zu\n", tokens.size(), tokenizer.vocab.size());
    fprintf(stderr, "[DEBUG] detokenize: first 10 token IDs: ");
    for (size_t i = 0; i < std::min(tokens.size(), (size_t)10); i++) {
        fprintf(stderr, "%d ", tokens[i]);
    }
    fprintf(stderr, "\n");
    fflush(stderr);

    // Detect encoding type for this tokenizer
    // Note: For simplicity, we detect on every call. Could cache per tokenizer if needed.
    bool use_gpt2_decoding = uses_gpt2_byte_encoding(tokenizer);

    int skipped_negative = 0;
    int skipped_oob = 0;
    int skipped_special = 0;
    int processed = 0;

    fprintf(stderr, "[DEBUG] detokenize: %zu tokens, vocab_size=%zu\n", tokens.size(), tokenizer.vocab.size());
    fflush(stderr);

    for (int32_t id : tokens) {
        if (id < 0) {
            skipped_negative++;
            continue;  // Skip invalid tokens
        }
        if (id >= static_cast<int32_t>(tokenizer.vocab.size())) {
            skipped_oob++;
            fprintf(stderr, "[DEBUG] detokenize: token %d out of vocab bounds (%zu)\n",
                    id, tokenizer.vocab.size());
            continue;
        }

        const std::string& token = tokenizer.vocab[id];

        // Skip special tokens in output
        if (id == tokenizer.bos_token_id ||
            id == tokenizer.eos_token_id ||
            id == tokenizer.pad_token_id) {
            skipped_special++;
            continue;
        }

        // Debug: show first few tokens being processed
        if (processed < 10) {
            fprintf(stderr, "[DEBUG] detokenize: id=%d -> token='%s' (len=%zu)\n",
                    id, token.c_str(), token.size());
        }

        // Handle byte tokens like <0xXX> (Llama-style byte fallback)
        if (token.size() == 6 && token.substr(0, 3) == "<0x" && token[5] == '>') {
            char hex[3] = {token[3], token[4], 0};
            unsigned int byte_val;
            if (sscanf(hex, "%x", &byte_val) == 1) {
                if (use_gpt2_decoding) {
                    // Convert byte to GPT-2 Unicode encoding for later decoding
                    accumulated += byte_to_unicode(static_cast<unsigned char>(byte_val));
                } else {
                    // Direct byte output
                    accumulated += static_cast<char>(byte_val);
                }
                processed++;
                continue;
            }
        }

        // Check if token is empty
        if (token.empty()) {
            fprintf(stderr, "[DEBUG] detokenize: WARNING id=%d has empty token\n", id);
            continue;
        }

        // Accumulate token
        accumulated += token;
        processed++;
    }

    fprintf(stderr, "[DEBUG] detokenize: processed=%d, skipped_neg=%d, skipped_oob=%d, skipped_special=%d\n",
            processed, skipped_negative, skipped_oob, skipped_special);
    fprintf(stderr, "[DEBUG] detokenize: accumulated len=%zu, use_gpt2_decoding=%d\n",
            accumulated.size(), use_gpt2_decoding);
    fflush(stderr);

    if (use_gpt2_decoding) {
        // Convert GPT-2 byte encoding back to original text
        result = gpt2_bytes_to_text(accumulated);
    } else {
        // Direct UTF-8 output (Qwen2, LLama3, etc.)
        result = accumulated;
    }

    fprintf(stderr, "[DEBUG] detokenize: final result len=%zu, first 50 chars='%.*s'\n",
            result.size(), (int)std::min(result.size(), (size_t)50), result.c_str());
    fflush(stderr);

    return true;
}

}  // namespace stcpp
