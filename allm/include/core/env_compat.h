#pragma once
/// @file env_compat.h
/// @brief Environment variable compatibility layer for ALLM_* and LLM_NODE_* migration
///
/// This header provides functions to read environment variables with backwards
/// compatibility. New ALLM_* variables take precedence over legacy LLM_NODE_* variables.

#include <cstdlib>
#include <string>

namespace allm {
namespace env {

/// Get an environment variable with ALLM_*/LLM_NODE_* fallback.
/// @param allm_name New ALLM_* variable name (without prefix)
/// @param legacy_name Legacy LLM_NODE_* variable name (without prefix), or nullptr to use allm_name
/// @return The value of the environment variable, or nullptr if not set
inline const char* get(const char* allm_name, const char* legacy_name = nullptr) {
    // Try new ALLM_* name first
    std::string new_var = std::string("ALLM_") + allm_name;
    if (const char* val = std::getenv(new_var.c_str())) {
        return val;
    }

    // Fall back to LLM_NODE_* (legacy)
    std::string legacy_var = std::string("LLM_NODE_") + (legacy_name ? legacy_name : allm_name);
    return std::getenv(legacy_var.c_str());
}

/// Check if an environment variable is set (ALLM_*/LLM_NODE_* with fallback).
inline bool is_set(const char* allm_name, const char* legacy_name = nullptr) {
    return get(allm_name, legacy_name) != nullptr;
}

/// Get an environment variable as a boolean (ALLM_*/LLM_NODE_* with fallback).
/// Returns true if the value is "1", "true", "yes", or "on" (case-insensitive).
inline bool get_bool(const char* allm_name, const char* legacy_name = nullptr, bool default_value = false) {
    const char* val = get(allm_name, legacy_name);
    if (!val) return default_value;

    std::string s(val);
    // Convert to lowercase for comparison
    for (auto& c : s) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return s == "1" || s == "true" || s == "yes" || s == "on";
}

/// Get an environment variable as an integer (ALLM_*/LLM_NODE_* with fallback).
inline int get_int(const char* allm_name, const char* legacy_name = nullptr, int default_value = 0) {
    const char* val = get(allm_name, legacy_name);
    if (!val) return default_value;

    try {
        return std::stoi(val);
    } catch (...) {
        return default_value;
    }
}

} // namespace env
} // namespace allm
