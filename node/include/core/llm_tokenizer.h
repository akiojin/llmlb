#pragma once

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <vector>

namespace llm_node {

struct LlmTokenizerHandle;

class LlmTokenizer {
public:
    static std::shared_ptr<LlmTokenizer> loadFromTokenizerJson(const std::string& tokenizer_json_path);

    ~LlmTokenizer();

    LlmTokenizer(const LlmTokenizer&) = delete;
    LlmTokenizer& operator=(const LlmTokenizer&) = delete;

    std::vector<int64_t> encode(const std::string& text, bool add_special_tokens) const;
    std::string decode(const std::vector<int64_t>& ids, bool skip_special_tokens) const;
    std::optional<int64_t> tokenToId(const std::string& token) const;

private:
    explicit LlmTokenizer(LlmTokenizerHandle* handle);

    LlmTokenizerHandle* handle_{nullptr};
};

}  // namespace llm_node

