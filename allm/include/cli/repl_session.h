#pragma once

#include <string>
#include <vector>
#include <functional>
#include <memory>
#include <nlohmann/json.hpp>

namespace allm {
namespace cli {

class CliClient;

/// Message role in conversation
enum class MessageRole {
    System,
    User,
    Assistant
};

/// Single message in conversation
struct Message {
    MessageRole role;
    std::string content;
    std::vector<std::string> images;  // Base64 encoded images for vision
    std::string thinking;             // Reasoning content (for deepseek-r1 etc.)
};

/// Session settings
struct SessionSettings {
    float temperature{0.7f};
    float top_p{0.9f};
    uint32_t max_tokens{0};  // 0 = unlimited
    bool show_thinking{false};
    bool stream{true};
};

/// REPL session for interactive model chat
class ReplSession {
public:
    /// Constructor
    /// @param client CLI client for server communication
    /// @param model_name Model to use
    /// @param settings Session settings
    ReplSession(
        std::shared_ptr<CliClient> client,
        const std::string& model_name,
        const SessionSettings& settings = {}
    );

    ~ReplSession();

    /// Run the REPL loop
    /// @return Exit code (0 = success, 1 = error, 2 = connection error)
    int run();

    /// Process a single user input
    /// @param input User input (may include image paths)
    /// @return true if session should continue, false if /bye
    bool processInput(const std::string& input);

    /// Clear conversation history (/clear command)
    void clearHistory();

    /// Get current conversation history
    const std::vector<Message>& getHistory() const { return history_; }

    /// Get model name
    const std::string& getModelName() const { return model_name_; }

    /// Get session settings
    const SessionSettings& getSettings() const { return settings_; }

    /// Update session settings
    void updateSettings(const SessionSettings& settings) { settings_ = settings; }

private:
    std::shared_ptr<CliClient> client_;
    std::string model_name_;
    SessionSettings settings_;
    std::vector<Message> history_;

    /// Parse user input for image paths and text
    /// @param input Raw user input
    /// @param images Output: extracted image paths
    /// @param text Output: text content
    void parseInput(const std::string& input, std::vector<std::string>& images, std::string& text);

    /// Load image file and encode as base64
    /// @param path Image file path
    /// @return Base64 encoded image data, or empty on error
    std::string loadImageAsBase64(const std::string& path);

    /// Extract thinking content from response
    /// @param response Full response text
    /// @param thinking Output: extracted thinking content
    /// @param content Output: content without thinking tags
    void extractThinking(const std::string& response, std::string& thinking, std::string& content);

    /// Build messages JSON for API request
    nlohmann::json buildMessagesJson() const;

    /// Print prompt
    void printPrompt();

    /// Read line from stdin
    std::string readLine();

    /// Handle stream chunk
    void handleStreamChunk(const std::string& chunk);
};

}  // namespace cli
}  // namespace allm
