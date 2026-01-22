#pragma once

#include <filesystem>
#include <optional>
#include <string>
#include <vector>

#include "core/engine_types.h"

namespace xllm {

// T166/T167: chat_template rendering via minja (Jinja2 for C++)
class ChatTemplateRenderer {
public:
    // Load chat_template from config.json in model directory
    // Returns nullopt if chat_template is not found
    static std::optional<ChatTemplateRenderer> fromConfigJson(
        const std::filesystem::path& model_dir);

    // Load chat_template from string directly
    static ChatTemplateRenderer fromString(
        const std::string& template_source,
        const std::string& bos_token = "",
        const std::string& eos_token = "");

    // Render messages to prompt string
    std::string render(
        const std::vector<ChatMessage>& messages,
        bool add_generation_prompt = true) const;

    // Get the raw template source
    const std::string& templateSource() const { return template_source_; }

    // Get BOS/EOS tokens
    const std::string& bosToken() const { return bos_token_; }
    const std::string& eosToken() const { return eos_token_; }

private:
    ChatTemplateRenderer(
        const std::string& template_source,
        const std::string& bos_token,
        const std::string& eos_token);

    std::string template_source_;
    std::string bos_token_;
    std::string eos_token_;
};

}  // namespace xllm
