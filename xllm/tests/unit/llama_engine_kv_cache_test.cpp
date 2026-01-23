#include <gtest/gtest.h>

#include <string>
#include <vector>

#include "core/llama_engine.h"

using namespace xllm;

TEST(LlamaEngineTest, KvCacheScopeTriggersResetHook) {
    std::vector<std::string> reasons;
    LlamaEngine::setKvCacheResetHookForTest([&](const char* reason) {
        reasons.push_back(reason ? reason : "");
    });

    LlamaEngine::runKvCacheScopeForTest();

    ASSERT_EQ(reasons.size(), 2u);
    EXPECT_EQ(reasons[0], "request_start");
    EXPECT_EQ(reasons[1], "request_end");

    LlamaEngine::setKvCacheResetHookForTest(nullptr);
}
