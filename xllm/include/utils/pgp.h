#pragma once

#include <filesystem>
#include <functional>
#include <string>

namespace xllm {

bool should_verify_pgp();

bool verify_pgp_signature(const std::filesystem::path& file_path,
                          const std::filesystem::path& signature_path,
                          std::string& error);

#ifdef XLLM_TESTING
using PgpVerifyHook = std::function<bool(const std::filesystem::path&,
                                         const std::filesystem::path&,
                                         std::string&)>;
void setPgpVerifyHookForTest(PgpVerifyHook hook);
#endif

}  // namespace xllm
