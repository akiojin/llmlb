#include "model_config.h"
#include <fstream>
#include <sstream>
#include <algorithm>
#include <cctype>

namespace nemotron {

namespace {

// Simple JSON parser for config.json (no external dependency)
class SimpleJsonParser {
public:
    explicit SimpleJsonParser(const std::string& json) : json_(json), pos_(0) {}

    bool parse(ModelConfig& config) {
        skipWhitespace();
        if (!expect('{')) return false;

        while (pos_ < json_.size()) {
            skipWhitespace();
            if (peek() == '}') {
                pos_++;
                return true;
            }

            std::string key = parseString();
            if (key.empty()) return false;

            skipWhitespace();
            if (!expect(':')) return false;
            skipWhitespace();

            if (!parseValue(key, config)) return false;

            skipWhitespace();
            if (peek() == ',') pos_++;
        }
        return false;
    }

private:
    const std::string& json_;
    size_t pos_;

    char peek() const { return pos_ < json_.size() ? json_[pos_] : '\0'; }
    void skipWhitespace() {
        while (pos_ < json_.size() && std::isspace(json_[pos_])) pos_++;
    }
    bool expect(char c) {
        if (peek() == c) { pos_++; return true; }
        return false;
    }

    std::string parseString() {
        if (!expect('"')) return "";
        size_t start = pos_;
        while (pos_ < json_.size() && json_[pos_] != '"') {
            if (json_[pos_] == '\\') pos_++;
            pos_++;
        }
        std::string result = json_.substr(start, pos_ - start);
        expect('"');
        return result;
    }

    int64_t parseNumber() {
        size_t start = pos_;
        if (peek() == '-') pos_++;
        while (pos_ < json_.size() && (std::isdigit(json_[pos_]) || json_[pos_] == '.')) {
            pos_++;
        }
        // Handle scientific notation
        if (peek() == 'e' || peek() == 'E') {
            pos_++;
            if (peek() == '+' || peek() == '-') pos_++;
            while (pos_ < json_.size() && std::isdigit(json_[pos_])) pos_++;
        }
        std::string numStr = json_.substr(start, pos_ - start);
        if (numStr.find('.') != std::string::npos || numStr.find('e') != std::string::npos) {
            return static_cast<int64_t>(std::stod(numStr));
        }
        return std::stoll(numStr);
    }

    double parseFloat() {
        size_t start = pos_;
        if (peek() == '-') pos_++;
        while (pos_ < json_.size() && (std::isdigit(json_[pos_]) || json_[pos_] == '.')) {
            pos_++;
        }
        if (peek() == 'e' || peek() == 'E') {
            pos_++;
            if (peek() == '+' || peek() == '-') pos_++;
            while (pos_ < json_.size() && std::isdigit(json_[pos_])) pos_++;
        }
        return std::stod(json_.substr(start, pos_ - start));
    }

    void skipValue() {
        skipWhitespace();
        char c = peek();
        if (c == '"') {
            parseString();
        } else if (c == '{') {
            int depth = 1;
            pos_++;
            while (pos_ < json_.size() && depth > 0) {
                if (json_[pos_] == '{') depth++;
                else if (json_[pos_] == '}') depth--;
                else if (json_[pos_] == '"') {
                    pos_++;
                    while (pos_ < json_.size() && json_[pos_] != '"') {
                        if (json_[pos_] == '\\') pos_++;
                        pos_++;
                    }
                }
                pos_++;
            }
        } else if (c == '[') {
            int depth = 1;
            pos_++;
            while (pos_ < json_.size() && depth > 0) {
                if (json_[pos_] == '[') depth++;
                else if (json_[pos_] == ']') depth--;
                else if (json_[pos_] == '"') {
                    pos_++;
                    while (pos_ < json_.size() && json_[pos_] != '"') {
                        if (json_[pos_] == '\\') pos_++;
                        pos_++;
                    }
                }
                pos_++;
            }
        } else {
            while (pos_ < json_.size() && json_[pos_] != ',' && json_[pos_] != '}') {
                pos_++;
            }
        }
    }

    bool parseValue(const std::string& key, ModelConfig& config) {
        if (key == "hidden_size") {
            config.hidden_size = static_cast<size_t>(parseNumber());
        } else if (key == "intermediate_size") {
            config.intermediate_size = static_cast<size_t>(parseNumber());
        } else if (key == "num_attention_heads") {
            config.num_attention_heads = static_cast<size_t>(parseNumber());
        } else if (key == "num_hidden_layers") {
            config.num_hidden_layers = static_cast<size_t>(parseNumber());
        } else if (key == "num_key_value_heads") {
            config.num_key_value_heads = static_cast<size_t>(parseNumber());
        } else if (key == "vocab_size") {
            config.vocab_size = static_cast<size_t>(parseNumber());
        } else if (key == "max_position_embeddings") {
            config.max_position_embeddings = static_cast<size_t>(parseNumber());
        } else if (key == "rms_norm_eps") {
            config.rms_norm_eps = static_cast<float>(parseFloat());
        } else if (key == "rope_theta") {
            config.rope_theta = static_cast<float>(parseFloat());
        } else if (key == "model_type") {
            config.model_type = parseString();
        } else {
            skipValue();
        }
        return true;
    }
};

}  // namespace

bool ModelConfig::validate(std::string& error) const {
    if (hidden_size == 0) {
        error = "hidden_size must be > 0";
        return false;
    }
    if (num_attention_heads == 0) {
        error = "num_attention_heads must be > 0";
        return false;
    }
    if (hidden_size % num_attention_heads != 0) {
        error = "hidden_size must be divisible by num_attention_heads";
        return false;
    }
    if (num_hidden_layers == 0) {
        error = "num_hidden_layers must be > 0";
        return false;
    }
    if (vocab_size == 0) {
        error = "vocab_size must be > 0";
        return false;
    }
    if (num_key_value_heads == 0 || num_attention_heads % num_key_value_heads != 0) {
        error = "num_attention_heads must be divisible by num_key_value_heads";
        return false;
    }
    return true;
}

ModelConfig loadModelConfig(const std::string& config_path) {
    std::ifstream file(config_path);
    if (!file.is_open()) {
        throw FileError("Cannot open config file: " + config_path);
    }

    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string json = buffer.str();

    ModelConfig config;
    SimpleJsonParser parser(json);
    if (!parser.parse(config)) {
        throw ModelError("Failed to parse config.json");
    }

    std::string error;
    if (!config.validate(error)) {
        throw ModelError("Invalid model config: " + error);
    }

    LOG_INFO("Model config loaded:");
    LOG_INFO("  hidden_size: " << config.hidden_size);
    LOG_INFO("  num_layers: " << config.num_hidden_layers);
    LOG_INFO("  num_heads: " << config.num_attention_heads);
    LOG_INFO("  num_kv_heads: " << config.num_key_value_heads);
    LOG_INFO("  vocab_size: " << config.vocab_size);

    return config;
}

}  // namespace nemotron
