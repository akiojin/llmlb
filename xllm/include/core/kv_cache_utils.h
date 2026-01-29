#pragma once

#include <filesystem>
#include <string>

namespace xllm {

std::string get_default_kv_cache_dir();

std::string build_kv_cache_key(const std::string& model_id, const std::string& prompt);

std::filesystem::path build_kv_cache_path(const std::string& model_id,
                                          const std::string& prompt,
                                          const std::string& base_dir = "");

bool ensure_kv_cache_dir(const std::string& dir, std::string& error);

}  // namespace xllm
