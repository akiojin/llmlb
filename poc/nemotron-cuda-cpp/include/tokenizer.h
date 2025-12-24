#pragma once

#include <string>
#include <vector>
#include <unordered_map>
#include <memory>
#include "config.h"

namespace nemotron {

// Simple BPE tokenizer for Nemotron
class Tokenizer {
public:
    Tokenizer() = default;

    // Load tokenizer from HuggingFace tokenizer.json
    void load(const std::string& tokenizer_path);

    // Encode text to token IDs
    std::vector<int32_t> encode(const std::string& text) const;

    // Decode token IDs to text
    std::string decode(const std::vector<int32_t>& token_ids) const;

    // Decode single token
    std::string decodeToken(int32_t token_id) const;

    // Special tokens
    int32_t getBosTokenId() const { return bos_token_id_; }
    int32_t getEosTokenId() const { return eos_token_id_; }
    int32_t getPadTokenId() const { return pad_token_id_; }

    // Vocabulary size
    size_t getVocabSize() const { return id_to_token_.size(); }

private:
    // Vocabulary
    std::unordered_map<std::string, int32_t> token_to_id_;
    std::unordered_map<int32_t, std::string> id_to_token_;

    // BPE merges: pair -> merged token
    std::vector<std::pair<std::string, std::string>> merges_;
    std::unordered_map<std::string, int> merge_ranks_;

    // Special token IDs
    int32_t bos_token_id_ = 1;
    int32_t eos_token_id_ = 2;
    int32_t pad_token_id_ = 0;
    int32_t unk_token_id_ = 0;

    // Internal BPE functions
    std::vector<std::string> bytesToTokens(const std::string& text) const;
    void applyBPE(std::vector<std::string>& tokens) const;
    std::string findBestPair(const std::vector<std::string>& tokens) const;

    // Byte-level encoding (GPT-style)
    static std::string byteToChar(uint8_t byte);
    static uint8_t charToByte(const std::string& c);
};

}  // namespace nemotron
