#include "utils/pgp.h"

#include <cctype>
#include <cstdlib>
#include <filesystem>
#include <string>

namespace fs = std::filesystem;

namespace xllm {

namespace {
bool parse_env_flag(const char* value, bool default_value) {
    if (!value) return default_value;
    std::string v(value);
    for (auto& c : v) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    if (v == "1" || v == "true" || v == "yes" || v == "on") return true;
    if (v == "0" || v == "false" || v == "no" || v == "off") return false;
    return default_value;
}

bool gpg_available() {
    int rc = std::system("gpg --version > /dev/null 2>&1");
    return rc == 0;
}

bool verify_with_gpg(const fs::path& file_path, const fs::path& signature_path, std::string& error) {
    if (!fs::exists(file_path)) {
        error = "PGP verify failed: file not found";
        return false;
    }
    if (!fs::exists(signature_path)) {
        error = "PGP verify failed: signature not found";
        return false;
    }
    if (!gpg_available()) {
        error = "PGP verify failed: gpg not available";
        return false;
    }
    std::string cmd = "gpg --verify \"" + signature_path.string() + "\" \"" + file_path.string() + "\" > /dev/null 2>&1";
    int rc = std::system(cmd.c_str());
    if (rc != 0) {
        error = "PGP verify failed: signature invalid";
        return false;
    }
    return true;
}

#ifdef XLLM_TESTING
PgpVerifyHook g_test_hook;
#endif
}  // namespace

bool should_verify_pgp() {
    return parse_env_flag(std::getenv("XLLM_PGP_VERIFY"), false);
}

bool verify_pgp_signature(const fs::path& file_path,
                          const fs::path& signature_path,
                          std::string& error) {
#ifdef XLLM_TESTING
    if (g_test_hook) {
        return g_test_hook(file_path, signature_path, error);
    }
#endif
    if (!should_verify_pgp()) {
        return true;
    }
    return verify_with_gpg(file_path, signature_path, error);
}

#ifdef XLLM_TESTING
void setPgpVerifyHookForTest(PgpVerifyHook hook) {
    g_test_hook = std::move(hook);
}
#endif

}  // namespace xllm
