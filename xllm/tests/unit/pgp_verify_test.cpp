#include <gtest/gtest.h>

#include <filesystem>
#include <string>

#include "utils/pgp.h"

namespace fs = std::filesystem;
using namespace xllm;

namespace {
class EnvGuard {
public:
    EnvGuard(const std::string& key, const std::string& value) : key_(key) {
        const char* prev = std::getenv(key.c_str());
        if (prev) {
            had_prev_ = true;
            prev_value_ = prev;
        }
#ifdef _WIN32
        _putenv_s(key.c_str(), value.c_str());
#else
        setenv(key.c_str(), value.c_str(), 1);
#endif
    }

    ~EnvGuard() {
#ifdef _WIN32
        if (had_prev_) {
            _putenv_s(key_.c_str(), prev_value_.c_str());
        } else {
            _putenv_s(key_.c_str(), "");
        }
#else
        if (had_prev_) {
            setenv(key_.c_str(), prev_value_.c_str(), 1);
        } else {
            unsetenv(key_.c_str());
        }
#endif
    }

private:
    std::string key_;
    bool had_prev_{false};
    std::string prev_value_;
};
}  // namespace

TEST(PgpVerifyTest, RespectsEnvToggle) {
    {
        EnvGuard env("XLLM_PGP_VERIFY", "0");
        EXPECT_FALSE(should_verify_pgp());
    }
    {
        EnvGuard env("XLLM_PGP_VERIFY", "true");
        EXPECT_TRUE(should_verify_pgp());
    }
}

TEST(PgpVerifyTest, UsesTestHookWhenProvided) {
    bool called = false;
    setPgpVerifyHookForTest([&](const fs::path& file, const fs::path& sig, std::string& error) {
        called = true;
        error = "forced";
        return false;
    });

    std::string error;
    bool ok = verify_pgp_signature("file.bin", "file.bin.sig", error);
    EXPECT_FALSE(ok);
    EXPECT_TRUE(called);
    EXPECT_EQ(error, "forced");

    setPgpVerifyHookForTest(nullptr);
}
