// SPEC-58378000: Session data model
// Represents a REPL interactive session

#pragma once

#include <string>
#include <vector>
#include <optional>
#include <chrono>

namespace xllm {
namespace cli {
namespace models {

/// Message role in conversation
enum class Role {
    User,
    Assistant,
    System
};

/// Image data for vision models
struct ImageData {
    /// Base64 encoded image data
    std::string data;

    /// MIME type (e.g., "image/png")
    std::string mime_type;

    /// Create from file path
    static ImageData fromFile(const std::string& path);

    /// Check if image is valid
    bool isValid() const { return !data.empty(); }
};

/// Single message in conversation history
struct Message {
    /// Message role
    Role role{Role::User};

    /// Text content
    std::string content;

    /// Image data for vision (optional)
    std::vector<ImageData> images;

    /// Thinking/reasoning content (for deepseek-r1 etc.)
    std::optional<std::string> thinking;

    /// Message timestamp
    std::chrono::system_clock::time_point timestamp;

    /// Create user message
    static Message user(const std::string& content);

    /// Create assistant message
    static Message assistant(const std::string& content);

    /// Create system message
    static Message system(const std::string& content);

    /// Get role as string
    static std::string roleToString(Role role);
};

/// Session settings
struct SessionSettings {
    /// Temperature (0.0-2.0)
    float temperature{0.7f};

    /// Top-p sampling (0.0-1.0)
    float top_p{0.9f};

    /// Maximum tokens (0 = unlimited)
    uint32_t max_tokens{0};

    /// Show thinking process (for reasoning models)
    bool show_thinking{false};

    /// Enable streaming output
    bool stream{true};
};

/// REPL interactive session
struct Session {
    /// Session ID (UUID)
    std::string id;

    /// Model name being used
    std::string model_name;

    /// Conversation history
    std::vector<Message> history;

    /// Session settings
    SessionSettings settings;

    /// Session start time
    std::chrono::system_clock::time_point created_at;

    /// Total token count
    uint64_t token_count{0};

    /// Clear conversation history
    void clearHistory();

    /// Add message to history
    void addMessage(const Message& message);

    /// Get total message count
    size_t messageCount() const { return history.size(); }

    /// Generate new session ID
    static std::string generateId();
};

}  // namespace models
}  // namespace cli
}  // namespace xllm
