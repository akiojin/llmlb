/**
 * @file safetensors_internal.h
 * @brief Internal header for safetensors.cpp implementation
 */

#ifndef SAFETENSORS_INTERNAL_H
#define SAFETENSORS_INTERNAL_H

#include "safetensors.h"
#include <string>
#include <vector>
#include <unordered_map>
#include <memory>
#include <cstdint>

namespace stcpp {

/* Constants */
constexpr size_t ST_HEADER_SIZE_LEN = 8;  // 8 bytes for header size (uint64 LE)
constexpr size_t MAX_TENSOR_DIMS = 8;      // Maximum tensor dimensions

/* Tensor data types */
enum class DType {
    F16,   // float16
    BF16,  // bfloat16
    F32,   // float32
    F64,   // float64
    I8,    // int8
    I16,   // int16
    I32,   // int32
    I64,   // int64
    U8,    // uint8
    U16,   // uint16
    U32,   // uint32
    U64,   // uint64
    BOOL,  // boolean
    UNKNOWN
};

/* Tensor metadata from safetensors header */
struct TensorInfo {
    std::string name;
    DType dtype;
    std::vector<int64_t> shape;
    size_t data_offset;  // Offset from start of data section
    size_t data_size;    // Size in bytes
};

/* Safetensors file header */
struct SafetensorsHeader {
    std::unordered_map<std::string, std::string> metadata;
    std::vector<TensorInfo> tensors;
    size_t header_size;
    size_t data_offset;  // Where tensor data begins
};

/* Model internal structure */
struct ModelImpl {
    std::string name;
    std::string model_path;
    std::vector<std::string> shard_paths;
    std::vector<SafetensorsHeader> shard_headers;

    // Model config (from config.json)
    int32_t n_layers = 0;
    int32_t n_heads = 0;
    int32_t hidden_size = 0;
    int32_t vocab_size = 0;
    int32_t max_context = 0;
    int32_t embedding_dims = 0;

    // TODO: ggml tensors will be stored here
};

/* Context internal structure */
struct ContextImpl {
    ModelImpl* model;
    stcpp_context_params params;

    // TODO: KV cache, ggml context, etc.
};

/* Tokenizer internal structure */
struct TokenizerImpl {
    std::vector<std::string> vocab;
    std::unordered_map<std::string, int32_t> vocab_to_id;
    int32_t bos_token_id = -1;
    int32_t eos_token_id = -1;
    int32_t pad_token_id = -1;
    std::string chat_template;

    // BPE merge rules
    std::vector<std::pair<std::string, std::string>> merges;
};

/* Utility functions */

// Convert dtype string to enum
DType str_to_dtype(const std::string& s);

// Get dtype size in bytes
size_t dtype_size(DType dtype);

// Read little-endian uint64
uint64_t read_u64_le(const uint8_t* data);

// Parse safetensors file header
bool parse_safetensors_header(
    const std::string& path,
    SafetensorsHeader& header,
    std::string& error
);

// Parse index.json for sharded models
bool parse_index_json(
    const std::string& path,
    std::vector<std::string>& shard_files,
    std::unordered_map<std::string, std::string>& tensor_to_shard,
    std::string& error
);

// Load model config from config.json
bool load_model_config(
    const std::string& model_dir,
    ModelImpl& model,
    std::string& error
);

// Load tokenizer from tokenizer.json
bool load_tokenizer(
    const std::string& model_dir,
    TokenizerImpl& tokenizer,
    std::string& error
);

}  // namespace stcpp

#endif  // SAFETENSORS_INTERNAL_H
