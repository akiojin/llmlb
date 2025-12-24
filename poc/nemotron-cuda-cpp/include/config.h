#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>
#include <memory>
#include <stdexcept>

namespace nemotron {

// Error handling
class NemotronError : public std::runtime_error {
public:
    explicit NemotronError(const std::string& msg) : std::runtime_error(msg) {}
};

class FileError : public NemotronError {
public:
    explicit FileError(const std::string& msg) : NemotronError("[FILE] " + msg) {}
};

class CudaError : public NemotronError {
public:
    explicit CudaError(const std::string& msg) : NemotronError("[CUDA] " + msg) {}
};

class ModelError : public NemotronError {
public:
    explicit ModelError(const std::string& msg) : NemotronError("[MODEL] " + msg) {}
};

// Data types
using f16 = uint16_t;  // FP16 stored as uint16
using bf16 = uint16_t; // BF16 stored as uint16
using f32 = float;
using i32 = int32_t;
using i64 = int64_t;
using u32 = uint32_t;
using u64 = uint64_t;

// Tensor shape type
using Shape = std::vector<size_t>;

// Logging macros
#ifndef NDEBUG
#define LOG_DEBUG(msg) std::cerr << "[DEBUG] " << msg << std::endl
#else
#define LOG_DEBUG(msg)
#endif

#define LOG_INFO(msg) std::cout << "[INFO] " << msg << std::endl
#define LOG_WARN(msg) std::cerr << "[WARN] " << msg << std::endl
#define LOG_ERROR(msg) std::cerr << "[ERROR] " << msg << std::endl

// Model configuration constants
constexpr size_t MAX_SEQ_LEN = 4096;
constexpr size_t MAX_BATCH_SIZE = 1;

}  // namespace nemotron
