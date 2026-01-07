/**
 * @file safetensors_loader.cpp
 * @brief Safetensors file format parser and loader
 */

#include "safetensors_internal.h"
#include <fstream>
#include <cstring>
#include <filesystem>
#include <set>

// Use nlohmann/json for JSON parsing (to be added as dependency)
// For now, we use a minimal JSON parser stub
namespace stcpp {

/* DType string conversion */
DType str_to_dtype(const std::string& s) {
    if (s == "F16") return DType::F16;
    if (s == "BF16") return DType::BF16;
    if (s == "F32") return DType::F32;
    if (s == "F64") return DType::F64;
    if (s == "I8") return DType::I8;
    if (s == "I16") return DType::I16;
    if (s == "I32") return DType::I32;
    if (s == "I64") return DType::I64;
    if (s == "U8") return DType::U8;
    if (s == "U16") return DType::U16;
    if (s == "U32") return DType::U32;
    if (s == "U64") return DType::U64;
    if (s == "BOOL") return DType::BOOL;
    return DType::UNKNOWN;
}

/* DType size in bytes */
size_t dtype_size(DType dtype) {
    switch (dtype) {
        case DType::F16:  return 2;
        case DType::BF16: return 2;
        case DType::F32:  return 4;
        case DType::F64:  return 8;
        case DType::I8:   return 1;
        case DType::I16:  return 2;
        case DType::I32:  return 4;
        case DType::I64:  return 8;
        case DType::U8:   return 1;
        case DType::U16:  return 2;
        case DType::U32:  return 4;
        case DType::U64:  return 8;
        case DType::BOOL: return 1;
        default:          return 0;
    }
}

/* Read little-endian uint64 */
uint64_t read_u64_le(const uint8_t* data) {
    return static_cast<uint64_t>(data[0])
         | (static_cast<uint64_t>(data[1]) << 8)
         | (static_cast<uint64_t>(data[2]) << 16)
         | (static_cast<uint64_t>(data[3]) << 24)
         | (static_cast<uint64_t>(data[4]) << 32)
         | (static_cast<uint64_t>(data[5]) << 40)
         | (static_cast<uint64_t>(data[6]) << 48)
         | (static_cast<uint64_t>(data[7]) << 56);
}

/* Minimal JSON parser helpers - to be replaced with proper JSON library */
namespace json_parser {

// Skip whitespace
void skip_ws(const char*& p, const char* end) {
    while (p < end && (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')) {
        ++p;
    }
}

// Parse string (returns empty on error)
std::string parse_string(const char*& p, const char* end) {
    skip_ws(p, end);
    if (p >= end || *p != '"') return "";
    ++p;  // skip opening quote

    std::string result;
    while (p < end && *p != '"') {
        if (*p == '\\' && p + 1 < end) {
            ++p;
            switch (*p) {
                case '"': result += '"'; break;
                case '\\': result += '\\'; break;
                case 'n': result += '\n'; break;
                case 't': result += '\t'; break;
                case 'r': result += '\r'; break;
                default: result += *p; break;
            }
        } else {
            result += *p;
        }
        ++p;
    }
    if (p < end) ++p;  // skip closing quote
    return result;
}

// Parse number (integer)
int64_t parse_int(const char*& p, const char* end) {
    skip_ws(p, end);
    bool negative = false;
    if (p < end && *p == '-') {
        negative = true;
        ++p;
    }
    int64_t result = 0;
    while (p < end && *p >= '0' && *p <= '9') {
        result = result * 10 + (*p - '0');
        ++p;
    }
    return negative ? -result : result;
}

// Skip to next field
void skip_value(const char*& p, const char* end) {
    skip_ws(p, end);
    if (p >= end) return;

    if (*p == '"') {
        parse_string(p, end);
    } else if (*p == '[') {
        int depth = 1;
        ++p;
        while (p < end && depth > 0) {
            if (*p == '[') ++depth;
            else if (*p == ']') --depth;
            else if (*p == '"') parse_string(p, end);
            else ++p;
        }
    } else if (*p == '{') {
        int depth = 1;
        ++p;
        while (p < end && depth > 0) {
            if (*p == '{') ++depth;
            else if (*p == '}') --depth;
            else if (*p == '"') parse_string(p, end);
            else ++p;
        }
    } else {
        while (p < end && *p != ',' && *p != '}' && *p != ']') {
            ++p;
        }
    }
}

}  // namespace json_parser

/* Parse safetensors file header */
bool parse_safetensors_header(
    const std::string& path,
    SafetensorsHeader& header,
    std::string& error
) {
    std::ifstream file(path, std::ios::binary);
    if (!file.is_open()) {
        error = "Failed to open file: " + path;
        return false;
    }

    // Get file size
    file.seekg(0, std::ios::end);
    size_t file_size = file.tellg();
    file.seekg(0, std::ios::beg);

    // Validate minimum size
    if (file_size <= ST_HEADER_SIZE_LEN) {
        error = "File too small to be valid safetensors: " + path;
        return false;
    }

    // Read header size (8 bytes, little-endian)
    uint8_t header_size_buf[ST_HEADER_SIZE_LEN];
    file.read(reinterpret_cast<char*>(header_size_buf), ST_HEADER_SIZE_LEN);
    if (!file) {
        error = "Failed to read header size: " + path;
        return false;
    }

    uint64_t header_size = read_u64_le(header_size_buf);
    if (header_size >= file_size - ST_HEADER_SIZE_LEN) {
        error = "Header size larger than file: " + path;
        return false;
    }

    header.header_size = header_size;
    header.data_offset = ST_HEADER_SIZE_LEN + header_size;

    // Read header JSON
    std::vector<char> header_buf(header_size + 1);
    file.read(header_buf.data(), header_size);
    if (!file) {
        error = "Failed to read header: " + path;
        return false;
    }
    header_buf[header_size] = '\0';

    // Parse JSON header
    const char* p = header_buf.data();
    const char* end = p + header_size;

    json_parser::skip_ws(p, end);
    if (p >= end || *p != '{') {
        error = "Invalid JSON header (expected '{')";
        return false;
    }
    ++p;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == '}') break;

        // Parse key
        std::string key = json_parser::parse_string(p, end);
        if (key.empty()) {
            error = "Invalid JSON: expected key";
            return false;
        }

        json_parser::skip_ws(p, end);
        if (p >= end || *p != ':') {
            error = "Invalid JSON: expected ':'";
            return false;
        }
        ++p;

        if (key == "__metadata__") {
            // Skip metadata for now
            json_parser::skip_value(p, end);
        } else {
            // Parse tensor info
            TensorInfo info;
            info.name = key;

            json_parser::skip_ws(p, end);
            if (p >= end || *p != '{') {
                error = "Invalid tensor info: expected '{'";
                return false;
            }
            ++p;

            while (p < end && *p != '}') {
                json_parser::skip_ws(p, end);
                std::string field = json_parser::parse_string(p, end);

                json_parser::skip_ws(p, end);
                if (p < end && *p == ':') ++p;

                if (field == "dtype") {
                    std::string dtype_str = json_parser::parse_string(p, end);
                    info.dtype = str_to_dtype(dtype_str);
                } else if (field == "shape") {
                    json_parser::skip_ws(p, end);
                    if (p < end && *p == '[') {
                        ++p;
                        while (p < end && *p != ']') {
                            json_parser::skip_ws(p, end);
                            if (p < end && *p >= '0' && *p <= '9') {
                                info.shape.push_back(json_parser::parse_int(p, end));
                            }
                            json_parser::skip_ws(p, end);
                            if (p < end && *p == ',') ++p;
                        }
                        if (p < end) ++p;  // skip ']'
                    }
                } else if (field == "data_offsets") {
                    json_parser::skip_ws(p, end);
                    if (p < end && *p == '[') {
                        ++p;
                        json_parser::skip_ws(p, end);
                        size_t begin_offset = json_parser::parse_int(p, end);
                        json_parser::skip_ws(p, end);
                        if (p < end && *p == ',') ++p;
                        json_parser::skip_ws(p, end);
                        size_t end_offset = json_parser::parse_int(p, end);
                        info.data_offset = begin_offset;
                        info.data_size = end_offset - begin_offset;
                        json_parser::skip_ws(p, end);
                        if (p < end && *p == ']') ++p;
                    }
                } else {
                    json_parser::skip_value(p, end);
                }

                json_parser::skip_ws(p, end);
                if (p < end && *p == ',') ++p;
            }
            if (p < end) ++p;  // skip '}'

            header.tensors.push_back(info);
        }

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }

    return true;
}

/* Parse index.json for sharded models */
bool parse_index_json(
    const std::string& path,
    std::vector<std::string>& shard_files,
    std::unordered_map<std::string, std::string>& tensor_to_shard,
    std::string& error
) {
    std::ifstream file(path);
    if (!file.is_open()) {
        error = "Failed to open index file: " + path;
        return false;
    }

    // Read entire file
    std::string content((std::istreambuf_iterator<char>(file)),
                         std::istreambuf_iterator<char>());

    const char* p = content.data();
    const char* end = p + content.size();

    json_parser::skip_ws(p, end);
    if (p >= end || *p != '{') {
        error = "Invalid index.json: expected '{'";
        return false;
    }
    ++p;

    std::set<std::string> unique_shards;

    while (p < end) {
        json_parser::skip_ws(p, end);
        if (p >= end || *p == '}') break;

        std::string key = json_parser::parse_string(p, end);

        json_parser::skip_ws(p, end);
        if (p < end && *p == ':') ++p;

        if (key == "weight_map") {
            json_parser::skip_ws(p, end);
            if (p < end && *p == '{') {
                ++p;
                while (p < end && *p != '}') {
                    json_parser::skip_ws(p, end);
                    std::string tensor_name = json_parser::parse_string(p, end);

                    json_parser::skip_ws(p, end);
                    if (p < end && *p == ':') ++p;

                    std::string shard_file = json_parser::parse_string(p, end);

                    if (!tensor_name.empty() && !shard_file.empty()) {
                        tensor_to_shard[tensor_name] = shard_file;
                        unique_shards.insert(shard_file);
                    }

                    json_parser::skip_ws(p, end);
                    if (p < end && *p == ',') ++p;
                }
                if (p < end) ++p;  // skip '}'
            }
        } else {
            json_parser::skip_value(p, end);
        }

        json_parser::skip_ws(p, end);
        if (p < end && *p == ',') ++p;
    }

    // Copy unique shards to vector
    shard_files.assign(unique_shards.begin(), unique_shards.end());

    if (shard_files.empty()) {
        error = "No shard files found in weight_map";
        return false;
    }

    return true;
}

}  // namespace stcpp
