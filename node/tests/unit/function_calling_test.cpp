// T168/T169/T178: Function Calling検出テスト
#include <gtest/gtest.h>

#include "core/function_calling.h"

using namespace llm_node;

class FunctionCallingDetectorTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Sample tool definitions for testing
        ToolDefinition weather_tool;
        weather_tool.name = "get_weather";
        weather_tool.description = "Get the current weather in a location";
        weather_tool.parameters = R"({
            "type": "object",
            "properties": {
                "location": {"type": "string", "description": "City name"},
                "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
            },
            "required": ["location"]
        })";

        ToolDefinition search_tool;
        search_tool.name = "search";
        search_tool.description = "Search the web";
        search_tool.parameters = R"({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        })";

        tools_ = {weather_tool, search_tool};
    }

    std::vector<ToolDefinition> tools_;
};

// T168: ツール定義のプロンプト埋め込み

TEST_F(FunctionCallingDetectorTest, FormatToolsAsPromptContainsToolNames) {
    FunctionCallingDetector detector(tools_);
    std::string prompt = detector.formatToolsAsPrompt();

    EXPECT_TRUE(prompt.find("get_weather") != std::string::npos);
    EXPECT_TRUE(prompt.find("search") != std::string::npos);
}

TEST_F(FunctionCallingDetectorTest, FormatToolsAsPromptContainsDescriptions) {
    FunctionCallingDetector detector(tools_);
    std::string prompt = detector.formatToolsAsPrompt();

    EXPECT_TRUE(prompt.find("Get the current weather") != std::string::npos);
    EXPECT_TRUE(prompt.find("Search the web") != std::string::npos);
}

TEST_F(FunctionCallingDetectorTest, FormatToolsAsPromptContainsParameters) {
    FunctionCallingDetector detector(tools_);
    std::string prompt = detector.formatToolsAsPrompt();

    EXPECT_TRUE(prompt.find("location") != std::string::npos);
    EXPECT_TRUE(prompt.find("query") != std::string::npos);
}

TEST_F(FunctionCallingDetectorTest, EmptyToolsReturnsEmptyPrompt) {
    FunctionCallingDetector detector({});
    std::string prompt = detector.formatToolsAsPrompt();

    EXPECT_TRUE(prompt.empty());
}

// T168: 出力からのJSON検出

TEST_F(FunctionCallingDetectorTest, DetectToolCallFromJsonOutput) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"(I will help you check the weather.

{"name": "get_weather", "arguments": {"location": "Tokyo", "unit": "celsius"}}
)";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->function_name, "get_weather");
    EXPECT_TRUE(result->arguments.find("Tokyo") != std::string::npos);
}

TEST_F(FunctionCallingDetectorTest, DetectToolCallWithCodeBlock) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"(Let me search for that.

```json
{"name": "search", "arguments": {"query": "weather forecast"}}
```
)";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->function_name, "search");
    EXPECT_TRUE(result->arguments.find("weather forecast") != std::string::npos);
}

TEST_F(FunctionCallingDetectorTest, DetectToolCallWithActionFormat) {
    // Some models use <tool_call> tags
    FunctionCallingDetector detector(tools_);

    std::string output = R"(<tool_call>
{"name": "get_weather", "arguments": {"location": "New York"}}
</tool_call>)";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->function_name, "get_weather");
}

TEST_F(FunctionCallingDetectorTest, NoToolCallReturnsNullopt) {
    FunctionCallingDetector detector(tools_);

    std::string output = "The weather in Tokyo is sunny with a high of 25°C.";

    auto result = detector.detectToolCall(output);

    EXPECT_FALSE(result.has_value());
}

TEST_F(FunctionCallingDetectorTest, InvalidJsonReturnsNullopt) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"({"name": "get_weather", "arguments": {"location": })";

    auto result = detector.detectToolCall(output);

    EXPECT_FALSE(result.has_value());
}

TEST_F(FunctionCallingDetectorTest, UnknownToolNameReturnsNullopt) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"({"name": "unknown_tool", "arguments": {}})";

    auto result = detector.detectToolCall(output);

    EXPECT_FALSE(result.has_value());
}

// T169: finish_reason="tool_calls"対応

TEST_F(FunctionCallingDetectorTest, DetectedToolCallHasGeneratedId) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"({"name": "get_weather", "arguments": {"location": "Tokyo"}})";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_FALSE(result->id.empty());
    EXPECT_TRUE(result->id.find("call_") == 0);  // ID starts with "call_"
}

TEST_F(FunctionCallingDetectorTest, ToolCallTypeIsFunction) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"({"name": "get_weather", "arguments": {"location": "Tokyo"}})";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->type, "function");
}

TEST_F(FunctionCallingDetectorTest, DetectToolCallPreservesArguments) {
    FunctionCallingDetector detector(tools_);

    std::string output = R"({"name": "get_weather", "arguments": {"location": "San Francisco", "unit": "fahrenheit"}})";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    // Arguments should be preserved as JSON string
    EXPECT_TRUE(result->arguments.find("San Francisco") != std::string::npos);
    EXPECT_TRUE(result->arguments.find("fahrenheit") != std::string::npos);
}

// ツール有効化チェック

TEST_F(FunctionCallingDetectorTest, HasToolsReturnsTrueWhenToolsDefined) {
    FunctionCallingDetector detector(tools_);
    EXPECT_TRUE(detector.hasTools());
}

TEST_F(FunctionCallingDetectorTest, HasToolsReturnsFalseWhenEmpty) {
    FunctionCallingDetector detector({});
    EXPECT_FALSE(detector.hasTools());
}

// OpenAI互換のfunction呼び出しフォーマット

TEST_F(FunctionCallingDetectorTest, DetectOpenAIFunctionCallFormat) {
    // OpenAI models might output in this format
    FunctionCallingDetector detector(tools_);

    std::string output = R"({
        "function_call": {
            "name": "get_weather",
            "arguments": "{\"location\": \"Tokyo\"}"
        }
    })";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->function_name, "get_weather");
}

// 複数のツール呼び出し（将来拡張用の基礎テスト）

TEST_F(FunctionCallingDetectorTest, DetectFirstToolCallWhenMultiple) {
    FunctionCallingDetector detector(tools_);

    // When output contains multiple tool calls, detect the first one
    std::string output = R"(
{"name": "get_weather", "arguments": {"location": "Tokyo"}}
{"name": "search", "arguments": {"query": "restaurants"}}
)";

    auto result = detector.detectToolCall(output);

    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->function_name, "get_weather");
}

