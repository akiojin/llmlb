#include "core/llm_tokenizer.h"

#include <filesystem>
#include <mutex>
#include <stdexcept>
#include <unordered_map>

namespace llm_node {

extern "C" {
void llm_tokenizer_free_string(char* s);
LlmTokenizerHandle* llm_tokenizer_load(const char* tokenizer_json_path, char** out_error);
void llm_tokenizer_free(LlmTokenizerHandle* handle);
void llm_tokenizer_free_i64_array(int64_t* ptr, size_t len);
bool llm_tokenizer_encode(
    const LlmTokenizerHandle* handle,
    const char* text,
    bool add_special_tokens,
    int64_t** out_ids_ptr,
    size_t* out_ids_len,
    char** out_error);
bool llm_tokenizer_decode(
    const LlmTokenizerHandle* handle,
    const int64_t* ids_ptr,
    size_t ids_len,
    bool skip_special_tokens,
    char** out_text,
    char** out_error);
int64_t llm_tokenizer_token_to_id(const LlmTokenizerHandle* handle, const char* token, char** out_error);
}  // extern "C"

namespace {

std::runtime_error makeError(const char* prefix, char* err) {
    std::string msg = prefix ? std::string(prefix) : std::string("error");
    if (err != nullptr) {
        msg += ": ";
        msg += err;
        llm_tokenizer_free_string(err);
    }
    return std::runtime_error(msg);
}

}  // namespace

LlmTokenizer::LlmTokenizer(LlmTokenizerHandle* handle) : handle_(handle) {
    if (handle_ == nullptr) {
        throw std::runtime_error("LlmTokenizer: null handle");
    }
}

LlmTokenizer::~LlmTokenizer() {
    if (handle_ != nullptr) {
        llm_tokenizer_free(handle_);
        handle_ = nullptr;
    }
}

std::shared_ptr<LlmTokenizer> LlmTokenizer::loadFromTokenizerJson(const std::string& tokenizer_json_path) {
    static std::mutex mu;
    static std::unordered_map<std::string, std::weak_ptr<LlmTokenizer>> cache;

    {
        std::lock_guard<std::mutex> lock(mu);
        auto it = cache.find(tokenizer_json_path);
        if (it != cache.end()) {
            if (auto sp = it->second.lock()) {
                return sp;
            }
        }
    }

    if (!std::filesystem::exists(tokenizer_json_path)) {
        throw std::runtime_error("tokenizer.json not found: " + tokenizer_json_path);
    }

    char* err = nullptr;
    LlmTokenizerHandle* handle = llm_tokenizer_load(tokenizer_json_path.c_str(), &err);
    if (handle == nullptr) {
        throw makeError("Failed to load tokenizer", err);
    }

    auto sp = std::shared_ptr<LlmTokenizer>(new LlmTokenizer(handle));
    {
        std::lock_guard<std::mutex> lock(mu);
        cache[tokenizer_json_path] = sp;
    }
    return sp;
}

std::vector<int64_t> LlmTokenizer::encode(const std::string& text, bool add_special_tokens) const {
    int64_t* ids_ptr = nullptr;
    size_t ids_len = 0;
    char* err = nullptr;
    if (!llm_tokenizer_encode(handle_, text.c_str(), add_special_tokens, &ids_ptr, &ids_len, &err)) {
        throw makeError("Tokenizer encode failed", err);
    }

    std::vector<int64_t> ids;
    ids.reserve(ids_len);
    for (size_t i = 0; i < ids_len; ++i) {
        ids.push_back(ids_ptr[i]);
    }
    llm_tokenizer_free_i64_array(ids_ptr, ids_len);
    return ids;
}

std::string LlmTokenizer::decode(const std::vector<int64_t>& ids, bool skip_special_tokens) const {
    char* out_text = nullptr;
    char* err = nullptr;
    const int64_t* ptr = ids.empty() ? nullptr : ids.data();
    if (!llm_tokenizer_decode(handle_, ptr, ids.size(), skip_special_tokens, &out_text, &err)) {
        throw makeError("Tokenizer decode failed", err);
    }

    std::string s = out_text ? std::string(out_text) : std::string();
    if (out_text != nullptr) {
        llm_tokenizer_free_string(out_text);
    }
    return s;
}

std::optional<int64_t> LlmTokenizer::tokenToId(const std::string& token) const {
    char* err = nullptr;
    const int64_t id = llm_tokenizer_token_to_id(handle_, token.c_str(), &err);
    if (err != nullptr) {
        llm_tokenizer_free_string(err);
    }
    if (id < 0) return std::nullopt;
    return id;
}

}  // namespace llm_node

