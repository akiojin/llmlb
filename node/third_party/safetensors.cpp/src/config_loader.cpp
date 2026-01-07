/**
 * @file config_loader.cpp
 * @brief Model config.json loader implementation (Task 26)
 */

#include "safetensors_internal.h"
#include <fstream>
#include <sstream>
#include <filesystem>

namespace stcpp {

// Forward declarations from json_parser namespace in safetensors_loader.cpp
namespace json_parser {
void skip_ws(const char*& p, const char* end);
std::string parse_string(const char*& p, const char* end);
int64_t parse_int(const char*& p, const char* end);
void skip_value(const char*& p, const char* end);
}

namespace {

// Parse a JSON object and extract model config fields
bool parse_config_json(const char* data, size_t len, ModelImpl& model, std::string& error) {
    const char* p = data;
    const char* end = data + len;

    json_parser::skip_ws(p, end);

    if (p >= end || *p != '{') {
        error = "Expected JSON object";
        return false;
    }
    p++;  // Skip '{'

    while (p < end) {
        json_parser::skip_ws(p, end);

        if (p >= end) break;
        if (*p == '}') break;

        // Parse key
        if (*p != '"') {
            error = "Expected string key";
            return false;
        }
        std::string key = json_parser::parse_string(p, end);

        json_parser::skip_ws(p, end);
        if (p >= end || *p != ':') {
            error = "Expected ':' after key";
            return false;
        }
        p++;  // Skip ':'

        json_parser::skip_ws(p, end);

        // Check for known keys and parse their values
        if (key == "vocab_size") {
            model.vocab_size = static_cast<int32_t>(json_parser::parse_int(p, end));
        } else if (key == "hidden_size" || key == "n_embd") {
            model.hidden_size = static_cast<int32_t>(json_parser::parse_int(p, end));
        } else if (key == "num_hidden_layers" || key == "n_layer") {
            model.n_layers = static_cast<int32_t>(json_parser::parse_int(p, end));
        } else if (key == "num_attention_heads" || key == "n_head") {
            model.n_heads = static_cast<int32_t>(json_parser::parse_int(p, end));
        } else if (key == "max_position_embeddings" || key == "n_positions") {
            model.max_context = static_cast<int32_t>(json_parser::parse_int(p, end));
        } else if (key == "hidden_dim" || key == "intermediate_size") {
            // Some models use these for embedding dimensions
            // Not setting for now as it varies by architecture
            json_parser::skip_value(p, end);
        } else {
            // Skip unknown keys
            json_parser::skip_value(p, end);
        }

        json_parser::skip_ws(p, end);

        // Handle comma or end of object
        if (p < end && *p == ',') {
            p++;
        }
    }

    return true;
}

}  // anonymous namespace

bool load_model_config(
    const std::string& model_dir,
    ModelImpl& model,
    std::string& error
) {
    namespace fs = std::filesystem;

    // Construct path to config.json
    fs::path config_path = fs::path(model_dir) / "config.json";

    // Check if file exists
    if (!fs::exists(config_path)) {
        error = "config.json not found in " + model_dir;
        return false;
    }

    // Read file contents
    std::ifstream file(config_path, std::ios::binary);
    if (!file.is_open()) {
        error = "Failed to open config.json";
        return false;
    }

    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string content = buffer.str();

    // Parse JSON
    if (!parse_config_json(content.data(), content.size(), model, error)) {
        return false;
    }

    // Set model path
    model.model_path = model_dir;

    return true;
}

}  // namespace stcpp
