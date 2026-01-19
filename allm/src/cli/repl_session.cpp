#include "cli/repl_session.h"
#include "cli/cli_client.h"
#include <iostream>
#include <sstream>
#include <fstream>

namespace allm {
namespace cli {

ReplSession::ReplSession(
    std::shared_ptr<CliClient> client,
    const std::string& model_name,
    const SessionSettings& settings
) : client_(std::move(client))
  , model_name_(model_name)
  , settings_(settings)
{
}

ReplSession::~ReplSession() = default;

int ReplSession::run() {
    // Check server connection
    if (!client_->isServerRunning()) {
        std::cerr << "Error: Could not connect to allm server" << std::endl;
        return 2;  // Connection error
    }

    std::cout << ">>> " << model_name_ << " (type /bye to exit, /clear to reset)" << std::endl;

    while (true) {
        printPrompt();
        std::string input = readLine();

        if (input.empty()) {
            continue;
        }

        if (!processInput(input)) {
            break;
        }
    }

    return 0;
}

bool ReplSession::processInput(const std::string& input) {
    // Handle commands
    if (input == "/bye") {
        std::cout << "Goodbye!" << std::endl;
        return false;
    }

    if (input == "/clear") {
        clearHistory();
        std::cout << "Conversation cleared." << std::endl;
        return true;
    }

    // Parse input for images and text
    std::vector<std::string> images;
    std::string text;
    parseInput(input, images, text);

    if (text.empty() && images.empty()) {
        return true;
    }

    // Add user message to history
    Message user_msg;
    user_msg.role = MessageRole::User;
    user_msg.content = text;
    user_msg.images = images;
    history_.push_back(user_msg);

    // Send to server
    auto messages_json = buildMessagesJson();
    auto response = client_->chat(
        model_name_,
        messages_json,
        settings_.stream ? [this](const std::string& chunk) {
            handleStreamChunk(chunk);
        } : StreamCallback{}
    );

    if (!response.ok()) {
        std::cerr << "Error: " << response.error_message << std::endl;
        history_.pop_back();  // Remove failed message
        return true;
    }

    // Add assistant response to history
    if (response.data) {
        std::string thinking, content;
        extractThinking(*response.data, thinking, content);

        Message assistant_msg;
        assistant_msg.role = MessageRole::Assistant;
        assistant_msg.content = content;
        assistant_msg.thinking = thinking;
        history_.push_back(assistant_msg);

        if (!settings_.stream) {
            if (settings_.show_thinking && !thinking.empty()) {
                std::cout << "<think>" << thinking << "</think>" << std::endl;
            }
            std::cout << content << std::endl;
        }
    }

    return true;
}

void ReplSession::clearHistory() {
    history_.clear();
}

void ReplSession::parseInput(
    const std::string& input,
    std::vector<std::string>& images,
    std::string& text
) {
    // Simple parsing: look for file paths ending in image extensions
    std::istringstream iss(input);
    std::string token;
    std::vector<std::string> text_parts;

    while (iss >> token) {
        // Check if it looks like an image path
        bool is_image = false;
        for (const auto& ext : {".jpg", ".jpeg", ".png", ".gif", ".webp"}) {
            if (token.length() > strlen(ext) &&
                token.substr(token.length() - strlen(ext)) == ext) {
                is_image = true;
                break;
            }
        }

        if (is_image) {
            std::string base64 = loadImageAsBase64(token);
            if (!base64.empty()) {
                images.push_back(base64);
            }
        } else {
            text_parts.push_back(token);
        }
    }

    // Reconstruct text
    std::ostringstream oss;
    for (size_t i = 0; i < text_parts.size(); ++i) {
        if (i > 0) oss << " ";
        oss << text_parts[i];
    }
    text = oss.str();
}

std::string ReplSession::loadImageAsBase64(const std::string& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        std::cerr << "Warning: Could not open image file: " << path << std::endl;
        return "";
    }

    // Read file content
    std::ostringstream oss;
    oss << file.rdbuf();
    std::string content = oss.str();

    // Base64 encode
    static const char* base64_chars =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    std::string encoded;
    int val = 0, valb = -6;
    for (unsigned char c : content) {
        val = (val << 8) + c;
        valb += 8;
        while (valb >= 0) {
            encoded.push_back(base64_chars[(val >> valb) & 0x3F]);
            valb -= 6;
        }
    }
    if (valb > -6) {
        encoded.push_back(base64_chars[((val << 8) >> (valb + 8)) & 0x3F]);
    }
    while (encoded.size() % 4) {
        encoded.push_back('=');
    }

    return encoded;
}

void ReplSession::extractThinking(
    const std::string& response,
    std::string& thinking,
    std::string& content
) {
    // Look for <think>...</think> tags
    const std::string open_tag = "<think>";
    const std::string close_tag = "</think>";

    size_t start = response.find(open_tag);
    size_t end = response.find(close_tag);

    if (start != std::string::npos && end != std::string::npos && end > start) {
        thinking = response.substr(start + open_tag.length(), end - start - open_tag.length());
        content = response.substr(0, start) + response.substr(end + close_tag.length());
    } else {
        thinking.clear();
        content = response;
    }
}

nlohmann::json ReplSession::buildMessagesJson() const {
    nlohmann::json messages = nlohmann::json::array();

    for (const auto& msg : history_) {
        nlohmann::json json_msg;

        switch (msg.role) {
            case MessageRole::System:
                json_msg["role"] = "system";
                break;
            case MessageRole::User:
                json_msg["role"] = "user";
                break;
            case MessageRole::Assistant:
                json_msg["role"] = "assistant";
                break;
        }

        if (msg.images.empty()) {
            json_msg["content"] = msg.content;
        } else {
            // Multi-modal content
            nlohmann::json content_array = nlohmann::json::array();

            // Add text part
            if (!msg.content.empty()) {
                content_array.push_back({
                    {"type", "text"},
                    {"text", msg.content}
                });
            }

            // Add image parts
            for (const auto& img : msg.images) {
                content_array.push_back({
                    {"type", "image_url"},
                    {"image_url", {
                        {"url", "data:image/jpeg;base64," + img}
                    }}
                });
            }

            json_msg["content"] = content_array;
        }

        messages.push_back(json_msg);
    }

    return messages;
}

void ReplSession::printPrompt() {
    std::cout << ">>> ";
    std::cout.flush();
}

std::string ReplSession::readLine() {
    std::string line;
    std::getline(std::cin, line);
    return line;
}

void ReplSession::handleStreamChunk(const std::string& chunk) {
    std::cout << chunk;
    std::cout.flush();
}

}  // namespace cli
}  // namespace allm
