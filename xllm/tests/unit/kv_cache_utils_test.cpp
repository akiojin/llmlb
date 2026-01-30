#include <gtest/gtest.h>

#include <filesystem>
#include <vector>

#include "core/kv_cache_utils.h"
#include "utils/sha256.h"

namespace fs = std::filesystem;

using namespace xllm;

TEST(KvCacheUtilsTest, BuildsDeterministicCachePath) {
    const std::string model_id = "/models/sample.gguf";
    const std::string prompt = "Hello";
    const std::string dir = "/tmp/xllm-cache";

    auto key = build_kv_cache_key(model_id, prompt);
    auto expected = fs::path(dir) / (key + ".session");
    auto actual = build_kv_cache_path(model_id, prompt, dir);

    EXPECT_EQ(actual, expected);
}

TEST(KvCacheUtilsTest, BuildKeyUsesSha256) {
    const std::string model_id = "model";
    const std::string prompt = "prompt";
    auto key = build_kv_cache_key(model_id, prompt);
    auto expected = sha256_text(model_id + "\n" + prompt);
    EXPECT_EQ(key, expected);
}
