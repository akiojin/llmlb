// SPEC-d7feaa2c: T168-T169, T178 Function Calling tests
#include <gtest/gtest.h>

#include "core/inference_engine.h"
#include "core/engine_types.h"

namespace llm_node {
namespace {

// T168: Tool prompt formatting tests
TEST(FunctionCallingTest, FormatToolsForPromptEmpty) {
    std::vector<ToolDefinition> tools;
    std::string result = formatToolsForPrompt(tools);
    EXPECT_TRUE(result.empty());
}

TEST(FunctionCallingTest, FormatToolsForPromptSingleTool) {
    std::vector<ToolDefinition> tools = {{
        "get_weather",
        "Get the current weather in a location",
        R"({"type":"object","properties":{"location":{"type":"string"}}})"
    }};

    std::string result = formatToolsForPrompt(tools);

    EXPECT_TRUE(result.find("get_weather") != std::string::npos);
    EXPECT_TRUE(result.find("Get the current weather") != std::string::npos);
    EXPECT_TRUE(result.find("location") != std::string::npos);
    EXPECT_TRUE(result.find("JSON") != std::string::npos);
}

TEST(FunctionCallingTest, FormatToolsForPromptMultipleTools) {
    std::vector<ToolDefinition> tools = {
        {"tool_a", "Description A", "{}"},
        {"tool_b", "Description B", "{}"},
        {"tool_c", "Description C", "{}"}
    };

    std::string result = formatToolsForPrompt(tools);

    EXPECT_TRUE(result.find("tool_a") != std::string::npos);
    EXPECT_TRUE(result.find("tool_b") != std::string::npos);
    EXPECT_TRUE(result.find("tool_c") != std::string::npos);
}

// T168: Tool call detection tests
TEST(FunctionCallingTest, DetectToolCallsEmpty) {
    std::string output = "Hello, how can I help you?";
    auto calls = detectToolCalls(output);
    EXPECT_TRUE(calls.empty());
}

TEST(FunctionCallingTest, DetectToolCallsSimple) {
    std::string output = R"(I'll get the weather for you.
{"name": "get_weather", "arguments": {"location": "Tokyo"}})";

    auto calls = detectToolCalls(output);

    ASSERT_EQ(calls.size(), 1u);
    EXPECT_EQ(calls[0].function_name, "get_weather");
    EXPECT_TRUE(calls[0].arguments_json.find("Tokyo") != std::string::npos);
    EXPECT_FALSE(calls[0].id.empty());
}

TEST(FunctionCallingTest, DetectToolCallsNestedJson) {
    std::string output = R"({"name": "search", "arguments": {"query": "test", "options": {"limit": 10}}})";

    auto calls = detectToolCalls(output);

    ASSERT_EQ(calls.size(), 1u);
    EXPECT_EQ(calls[0].function_name, "search");
    EXPECT_TRUE(calls[0].arguments_json.find("limit") != std::string::npos);
}

TEST(FunctionCallingTest, DetectToolCallsMultiple) {
    std::string output = R"(
{"name": "tool_a", "arguments": {}}
Some text
{"name": "tool_b", "arguments": {"x": 1}}
)";

    auto calls = detectToolCalls(output);

    ASSERT_EQ(calls.size(), 2u);
    EXPECT_EQ(calls[0].function_name, "tool_a");
    EXPECT_EQ(calls[1].function_name, "tool_b");
}

TEST(FunctionCallingTest, DetectToolCallsIgnoresRegularJson) {
    // JSON without "name" field should not be detected as tool call
    std::string output = R"({"city": "Tokyo", "temperature": 25})";

    auto calls = detectToolCalls(output);
    EXPECT_TRUE(calls.empty());
}

TEST(FunctionCallingTest, DetectToolCallsHandlesMalformedJson) {
    std::string output = R"({"name": "test" incomplete json)";

    auto calls = detectToolCalls(output);
    EXPECT_TRUE(calls.empty());
}

TEST(FunctionCallingTest, DetectToolCallsGeneratesUniqueIds) {
    std::string output = R"({"name": "a", "arguments": {}}{"name": "b", "arguments": {}})";

    auto calls = detectToolCalls(output);

    ASSERT_EQ(calls.size(), 2u);
    EXPECT_NE(calls[0].id, calls[1].id);
}

// T169: Tool calls in InferenceParams
TEST(FunctionCallingTest, InferenceParamsHasTools) {
    InferenceParams params;
    EXPECT_TRUE(params.tools.empty());

    params.tools.push_back({"test_tool", "A test tool", "{}"});
    EXPECT_EQ(params.tools.size(), 1u);
    EXPECT_EQ(params.tools[0].name, "test_tool");
}

}  // namespace
}  // namespace llm_node
