#pragma once

#include <optional>
#include <string>
#include <vector>

namespace allm {

/// T168: ツール定義（OpenAI互換）
struct ToolDefinition {
    std::string name;
    std::string description;
    std::string parameters;  // JSON Schema string
};

/// T169: ツール呼び出し結果（OpenAI互換）
struct ToolCall {
    std::string id;            // 例: "call_abc123"
    std::string type;          // 常に "function"
    std::string function_name;
    std::string arguments;     // JSON string
};

/// T168/T169: Function Calling検出クラス
class FunctionCallingDetector {
public:
    explicit FunctionCallingDetector(const std::vector<ToolDefinition>& tools);

    /// ツール定義をプロンプト用文字列に変換
    std::string formatToolsAsPrompt() const;

    /// モデル出力からツール呼び出しを検出
    /// @param output モデルの生成出力
    /// @return 検出されたツール呼び出し（なければnullopt）
    std::optional<ToolCall> detectToolCall(const std::string& output) const;

    /// ツールが定義されているか
    bool hasTools() const { return !tools_.empty(); }

    /// 定義されたツール一覧を取得
    const std::vector<ToolDefinition>& tools() const { return tools_; }

private:
    std::vector<ToolDefinition> tools_;

    /// ツール名がツールリストに存在するか確認
    bool isValidToolName(const std::string& name) const;

    /// 一意のツール呼び出しIDを生成
    static std::string generateToolCallId();
};

}  // namespace allm
