#include <gtest/gtest.h>

#include <nlohmann/json.hpp>

#include "utils/utf8.h"

using namespace llm_node;

TEST(Utf8UtilsTest, SanitizeUtf8LossyPassesValidUtf8Through) {
    std::string s = "hello \xE2\x98\x83";
    EXPECT_EQ(sanitize_utf8_lossy(s), s);
}

TEST(Utf8UtilsTest, SanitizeUtf8LossyReplacesInvalidBytes) {
    std::string bad = "ok";
    bad.push_back(static_cast<char>(0x84));  // invalid continuation byte as a start byte
    bad += "x";

    const std::string sanitized = sanitize_utf8_lossy(bad);

    EXPECT_NO_THROW({
        nlohmann::json j;
        j["content"] = sanitized;
        (void)j.dump();
    });
    EXPECT_NE(sanitized.find("\xEF\xBF\xBD"), std::string::npos);
}

TEST(Utf8UtilsTest, SanitizeUtf8LossyHandlesTruncatedSequence) {
    std::string bad;
    bad.push_back(static_cast<char>(0xE2));
    bad.push_back(static_cast<char>(0x82));

    const std::string sanitized = sanitize_utf8_lossy(bad);

    EXPECT_NO_THROW({
        nlohmann::json j;
        j["content"] = sanitized;
        (void)j.dump();
    });
    EXPECT_NE(sanitized.find("\xEF\xBF\xBD"), std::string::npos);
}
