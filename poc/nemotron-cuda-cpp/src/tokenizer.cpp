#include "tokenizer.h"
#include <algorithm>
#include <fstream>
#include <regex>
#include <sstream>
#include <unordered_map>

namespace nemotron {

namespace {

// GPT-style byte-to-unicode mapping - built at runtime to avoid UTF-8 encoding issues
// Maps bytes 0-255 to printable Unicode characters
// For printable ASCII (32-126 except special chars), maps to itself
// For non-printable bytes, maps to Unicode starting at U+0100
class ByteEncoder {
public:
    ByteEncoder() {
        int n = 0;
        for (int b = 0; b < 256; ++b) {
            if ((b >= 33 && b <= 126 && b != 92) ||  // Printable ASCII except backslash
                (b >= 161 && b <= 172) ||
                (b >= 174 && b <= 255)) {
                // Use the byte itself as a character
                encoder_[b] = std::string(1, static_cast<char>(b));
            } else {
                // Map to Unicode character starting at U+0100
                // UTF-8 encode the codepoint (U+0100 + n)
                int codepoint = 0x100 + n;
                encoder_[b] = encodeUtf8(codepoint);
                n++;
            }
        }
        // Build reverse mapping
        for (int b = 0; b < 256; ++b) {
            decoder_[encoder_[b]] = static_cast<uint8_t>(b);
        }
    }

    const std::string& encode(uint8_t byte) const { return encoder_[byte]; }

    bool decode(const std::string& s, size_t pos, uint8_t& out, size_t& len) const {
        // Try to match encoded strings (longest first)
        for (size_t l = 3; l >= 1; --l) {
            if (pos + l <= s.size()) {
                std::string sub = s.substr(pos, l);
                auto it = decoder_.find(sub);
                if (it != decoder_.end()) {
                    out = it->second;
                    len = l;
                    return true;
                }
            }
        }
        return false;
    }

private:
    static std::string encodeUtf8(int codepoint) {
        std::string result;
        if (codepoint < 0x80) {
            result += static_cast<char>(codepoint);
        } else if (codepoint < 0x800) {
            result += static_cast<char>(0xC0 | (codepoint >> 6));
            result += static_cast<char>(0x80 | (codepoint & 0x3F));
        } else if (codepoint < 0x10000) {
            result += static_cast<char>(0xE0 | (codepoint >> 12));
            result += static_cast<char>(0x80 | ((codepoint >> 6) & 0x3F));
            result += static_cast<char>(0x80 | (codepoint & 0x3F));
        }
        return result;
    }

    std::string encoder_[256];
    std::unordered_map<std::string, uint8_t> decoder_;
};

// Global byte encoder instance
const ByteEncoder& getByteEncoder() {
    static ByteEncoder instance;
    return instance;
}

// Simple JSON string extraction
std::string extractJsonString(const std::string& json, const std::string& key) {
    std::string searchKey = "\"" + key + "\"";
    size_t pos = json.find(searchKey);
    if (pos == std::string::npos) return "";

    pos = json.find(':', pos);
    if (pos == std::string::npos) return "";

    pos = json.find('"', pos + 1);
    if (pos == std::string::npos) return "";

    size_t start = pos + 1;
    size_t end = start;
    while (end < json.size() && json[end] != '"') {
        if (json[end] == '\\') end++;
        end++;
    }

    return json.substr(start, end - start);
}

int64_t extractJsonInt(const std::string& json, const std::string& key) {
    std::string searchKey = "\"" + key + "\"";
    size_t pos = json.find(searchKey);
    if (pos == std::string::npos) return -1;

    pos = json.find(':', pos);
    if (pos == std::string::npos) return -1;

    pos++;
    while (pos < json.size() && std::isspace(json[pos])) pos++;

    size_t start = pos;
    while (pos < json.size() && (std::isdigit(json[pos]) || json[pos] == '-')) pos++;

    if (start == pos) return -1;
    return std::stoll(json.substr(start, pos - start));
}

}  // namespace

std::string Tokenizer::byteToChar(uint8_t byte) {
    return getByteEncoder().encode(byte);
}

void Tokenizer::load(const std::string& tokenizer_path) {
    std::ifstream file(tokenizer_path);
    if (!file.is_open()) {
        throw FileError("Cannot open tokenizer file: " + tokenizer_path);
    }

    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string json = buffer.str();

    LOG_INFO("Loading tokenizer from: " << tokenizer_path);

    // Parse vocabulary from "model" -> "vocab"
    size_t vocab_pos = json.find("\"vocab\"");
    if (vocab_pos != std::string::npos) {
        size_t brace_start = json.find('{', vocab_pos);
        if (brace_start != std::string::npos) {
            int depth = 1;
            size_t pos = brace_start + 1;
            while (pos < json.size() && depth > 0) {
                if (json[pos] == '{') depth++;
                else if (json[pos] == '}') depth--;
                else if (json[pos] == '"' && depth == 1) {
                    // Parse token
                    size_t token_start = pos + 1;
                    size_t token_end = token_start;
                    while (token_end < json.size() && json[token_end] != '"') {
                        if (json[token_end] == '\\') token_end++;
                        token_end++;
                    }
                    std::string token = json.substr(token_start, token_end - token_start);

                    // Find ID
                    size_t colon = json.find(':', token_end);
                    if (colon != std::string::npos) {
                        size_t id_start = colon + 1;
                        while (id_start < json.size() && std::isspace(json[id_start])) id_start++;
                        size_t id_end = id_start;
                        while (id_end < json.size() && std::isdigit(json[id_end])) id_end++;
                        if (id_end > id_start) {
                            int32_t id = std::stoi(json.substr(id_start, id_end - id_start));
                            token_to_id_[token] = id;
                            id_to_token_[id] = token;
                        }
                        pos = id_end;
                    }
                }
                pos++;
            }
        }
    }

    // Parse merges from "model" -> "merges"
    size_t merges_pos = json.find("\"merges\"");
    if (merges_pos != std::string::npos) {
        size_t bracket_start = json.find('[', merges_pos);
        if (bracket_start != std::string::npos) {
            size_t pos = bracket_start + 1;
            int rank = 0;
            while (pos < json.size() && json[pos] != ']') {
                if (json[pos] == '"') {
                    size_t str_start = pos + 1;
                    size_t str_end = str_start;
                    while (str_end < json.size() && json[str_end] != '"') {
                        if (json[str_end] == '\\') str_end++;
                        str_end++;
                    }
                    std::string merge = json.substr(str_start, str_end - str_start);

                    // Split by space
                    size_t space = merge.find(' ');
                    if (space != std::string::npos) {
                        std::string first = merge.substr(0, space);
                        std::string second = merge.substr(space + 1);
                        merges_.emplace_back(first, second);
                        merge_ranks_[first + " " + second] = rank++;
                    }
                    pos = str_end + 1;
                } else {
                    pos++;
                }
            }
        }
    }

    // Parse special tokens
    // Look for bos_token, eos_token in added_tokens
    size_t added_tokens_pos = json.find("\"added_tokens\"");
    if (added_tokens_pos != std::string::npos) {
        // Simple parsing for special tokens
        size_t bos_pos = json.find("\"<s>\"", added_tokens_pos);
        if (bos_pos != std::string::npos) {
            int64_t id = extractJsonInt(json.substr(bos_pos - 50, 100), "id");
            if (id >= 0) bos_token_id_ = static_cast<int32_t>(id);
        }

        size_t eos_pos = json.find("\"</s>\"", added_tokens_pos);
        if (eos_pos != std::string::npos) {
            int64_t id = extractJsonInt(json.substr(eos_pos - 50, 100), "id");
            if (id >= 0) eos_token_id_ = static_cast<int32_t>(id);
        }
    }

    // Fallback: try to find in vocabulary
    auto bos_it = token_to_id_.find("<s>");
    if (bos_it != token_to_id_.end()) bos_token_id_ = bos_it->second;

    auto eos_it = token_to_id_.find("</s>");
    if (eos_it != token_to_id_.end()) eos_token_id_ = eos_it->second;

    LOG_INFO("  Vocabulary size: " << token_to_id_.size());
    LOG_INFO("  Merge rules: " << merges_.size());
    LOG_INFO("  BOS token ID: " << bos_token_id_);
    LOG_INFO("  EOS token ID: " << eos_token_id_);
}

std::vector<std::string> Tokenizer::bytesToTokens(const std::string& text) const {
    std::vector<std::string> tokens;
    for (unsigned char c : text) {
        tokens.push_back(byteToChar(c));
    }
    return tokens;
}

std::string Tokenizer::findBestPair(const std::vector<std::string>& tokens) const {
    std::string best_pair;
    int best_rank = INT_MAX;

    for (size_t i = 0; i + 1 < tokens.size(); ++i) {
        std::string pair = tokens[i] + " " + tokens[i + 1];
        auto it = merge_ranks_.find(pair);
        if (it != merge_ranks_.end() && it->second < best_rank) {
            best_rank = it->second;
            best_pair = pair;
        }
    }

    return best_pair;
}

void Tokenizer::applyBPE(std::vector<std::string>& tokens) const {
    while (tokens.size() > 1) {
        std::string best_pair = findBestPair(tokens);
        if (best_pair.empty()) break;

        size_t space = best_pair.find(' ');
        std::string first = best_pair.substr(0, space);
        std::string second = best_pair.substr(space + 1);
        std::string merged = first + second;

        std::vector<std::string> new_tokens;
        for (size_t i = 0; i < tokens.size(); ++i) {
            if (i + 1 < tokens.size() && tokens[i] == first && tokens[i + 1] == second) {
                new_tokens.push_back(merged);
                i++;  // Skip next token
            } else {
                new_tokens.push_back(tokens[i]);
            }
        }
        tokens = std::move(new_tokens);
    }
}

std::vector<int32_t> Tokenizer::encode(const std::string& text) const {
    std::vector<int32_t> result;

    // Add BOS token
    result.push_back(bos_token_id_);

    if (text.empty()) {
        return result;
    }

    // Convert to byte-level tokens
    std::vector<std::string> tokens = bytesToTokens(text);

    // Apply BPE
    applyBPE(tokens);

    // Convert to IDs
    for (const auto& token : tokens) {
        auto it = token_to_id_.find(token);
        if (it != token_to_id_.end()) {
            result.push_back(it->second);
        } else {
            // Unknown token - try character by character
            for (char c : token) {
                std::string single(1, c);
                auto char_it = token_to_id_.find(single);
                if (char_it != token_to_id_.end()) {
                    result.push_back(char_it->second);
                } else {
                    result.push_back(unk_token_id_);
                }
            }
        }
    }

    return result;
}

std::string Tokenizer::decodeToken(int32_t token_id) const {
    auto it = id_to_token_.find(token_id);
    if (it == id_to_token_.end()) {
        return "";
    }

    const std::string& token = it->second;
    std::string result;
    const ByteEncoder& encoder = getByteEncoder();

    // Decode byte-level encoding
    size_t i = 0;
    while (i < token.size()) {
        uint8_t byte;
        size_t len;
        if (encoder.decode(token, i, byte, len)) {
            result += static_cast<char>(byte);
            i += len;
        } else {
            // Direct character
            result += token[i];
            i++;
        }
    }

    return result;
}

std::string Tokenizer::decode(const std::vector<int32_t>& token_ids) const {
    std::string result;
    for (int32_t id : token_ids) {
        // Skip special tokens
        if (id == bos_token_id_ || id == eos_token_id_ || id == pad_token_id_) {
            continue;
        }
        result += decodeToken(id);
    }
    return result;
}

}  // namespace nemotron
