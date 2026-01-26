#include "core/function_calling.h"

#include <nlohmann/json.hpp>
#include <random>
#include <regex>
#include <sstream>

namespace xllm {

FunctionCallingDetector::FunctionCallingDetector(const std::vector<ToolDefinition>& tools)
    : tools_(tools) {}

std::string FunctionCallingDetector::formatToolsAsPrompt() const {
    if (tools_.empty()) {
        return "";
    }

    std::ostringstream ss;
    ss << "You have access to the following tools:\n\n";

    for (const auto& tool : tools_) {
        ss << "### " << tool.name << "\n";
        ss << tool.description << "\n";
        ss << "Parameters: " << tool.parameters << "\n\n";
    }

    ss << "To use a tool, respond with a JSON object in the following format:\n";
    ss << R"({"name": "tool_name", "arguments": {"param1": "value1", ...}})";
    ss << "\n";

    return ss.str();
}

std::optional<ToolCall> FunctionCallingDetector::detectToolCall(const std::string& output) const {
    if (tools_.empty()) {
        return std::nullopt;
    }

    // Try to find JSON in the output
    // Pattern 1: Raw JSON object
    // Pattern 2: JSON in code block (```json ... ```)
    // Pattern 3: JSON in <tool_call> tags
    // Pattern 4: OpenAI function_call format

    std::string json_str;

    // Try to extract from <tool_call> tags first
    std::regex tool_call_regex(R"(<tool_call>\s*([\s\S]*?)\s*</tool_call>)");
    std::smatch tool_call_match;
    if (std::regex_search(output, tool_call_match, tool_call_regex)) {
        json_str = tool_call_match[1].str();
    }

    // Try to extract from code blocks
    if (json_str.empty()) {
        std::regex code_block_regex(R"(```(?:json)?\s*([\s\S]*?)\s*```)");
        std::smatch code_match;
        if (std::regex_search(output, code_match, code_block_regex)) {
            json_str = code_match[1].str();
        }
    }

    // Try to find nested JSON by brace matching
    if (json_str.empty()) {
        // Find the first { and its matching }
        size_t start = output.find('{');
        if (start != std::string::npos) {
            int depth = 0;
            size_t end = start;
            for (size_t i = start; i < output.length(); ++i) {
                if (output[i] == '{') {
                    depth++;
                } else if (output[i] == '}') {
                    depth--;
                    if (depth == 0) {
                        end = i + 1;
                        break;
                    }
                }
            }
            if (depth == 0 && end > start) {
                json_str = output.substr(start, end - start);
            }
        }
    }

    if (json_str.empty()) {
        return std::nullopt;
    }

    // Parse the JSON
    nlohmann::json parsed;
    try {
        parsed = nlohmann::json::parse(json_str);
    } catch (const nlohmann::json::exception&) {
        return std::nullopt;
    }

    std::string tool_name;
    std::string arguments;

    // Handle OpenAI function_call format
    if (parsed.contains("function_call")) {
        auto& func = parsed["function_call"];
        if (func.contains("name") && func["name"].is_string()) {
            tool_name = func["name"].get<std::string>();
        }
        if (func.contains("arguments")) {
            if (func["arguments"].is_string()) {
                arguments = func["arguments"].get<std::string>();
            } else {
                arguments = func["arguments"].dump();
            }
        }
    }
    // Handle standard format: {"name": "tool", "arguments": {...}}
    else if (parsed.contains("name") && parsed["name"].is_string()) {
        tool_name = parsed["name"].get<std::string>();
        if (parsed.contains("arguments")) {
            if (parsed["arguments"].is_object()) {
                arguments = parsed["arguments"].dump();
            } else if (parsed["arguments"].is_string()) {
                arguments = parsed["arguments"].get<std::string>();
            }
        }
    }

    if (tool_name.empty()) {
        return std::nullopt;
    }

    // Validate the tool name
    if (!isValidToolName(tool_name)) {
        return std::nullopt;
    }

    ToolCall result;
    result.id = generateToolCallId();
    result.type = "function";
    result.function_name = tool_name;
    result.arguments = arguments;

    return result;
}

bool FunctionCallingDetector::isValidToolName(const std::string& name) const {
    for (const auto& tool : tools_) {
        if (tool.name == name) {
            return true;
        }
    }
    return false;
}

std::string FunctionCallingDetector::generateToolCallId() {
    static const char charset[] =
        "0123456789"
        "abcdefghijklmnopqrstuvwxyz"
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    static std::random_device rd;
    static std::mt19937 gen(rd());
    static std::uniform_int_distribution<size_t> dist(0, sizeof(charset) - 2);

    std::string id = "call_";
    for (int i = 0; i < 24; ++i) {
        id += charset[dist(gen)];
    }
    return id;
}

}  // namespace xllm
