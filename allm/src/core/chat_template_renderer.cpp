#include "core/chat_template_renderer.h"

#include <fstream>
#include <sstream>

#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

// T166: Include minja from llama.cpp vendor
#include "minja/minja.hpp"

namespace fs = std::filesystem;
using json = nlohmann::ordered_json;
using minja::Value;

namespace allm {

ChatTemplateRenderer::ChatTemplateRenderer(
    const std::string& template_source,
    const std::string& bos_token,
    const std::string& eos_token)
    : template_source_(template_source)
    , bos_token_(bos_token)
    , eos_token_(eos_token) {}

std::optional<ChatTemplateRenderer> ChatTemplateRenderer::fromConfigJson(
    const fs::path& model_dir) {

    const fs::path config_path = model_dir / "config.json";
    if (!fs::exists(config_path)) {
        spdlog::debug("config.json not found: {}", config_path.string());
        return std::nullopt;
    }

    try {
        std::ifstream file(config_path);
        if (!file.is_open()) {
            spdlog::warn("Failed to open config.json: {}", config_path.string());
            return std::nullopt;
        }

        json config = json::parse(file);

        // Check for chat_template field
        if (!config.contains("chat_template") || !config["chat_template"].is_string()) {
            spdlog::debug("No chat_template found in config.json");
            return std::nullopt;
        }

        std::string template_source = config["chat_template"].get<std::string>();

        // Get optional BOS/EOS tokens
        std::string bos_token;
        std::string eos_token;

        if (config.contains("bos_token")) {
            if (config["bos_token"].is_string()) {
                bos_token = config["bos_token"].get<std::string>();
            } else if (config["bos_token"].is_object() && config["bos_token"].contains("content")) {
                bos_token = config["bos_token"]["content"].get<std::string>();
            }
        }

        if (config.contains("eos_token")) {
            if (config["eos_token"].is_string()) {
                eos_token = config["eos_token"].get<std::string>();
            } else if (config["eos_token"].is_object() && config["eos_token"].contains("content")) {
                eos_token = config["eos_token"]["content"].get<std::string>();
            }
        }

        spdlog::debug("Loaded chat_template from config.json: {} chars, bos='{}', eos='{}'",
                      template_source.size(), bos_token, eos_token);

        return ChatTemplateRenderer(template_source, bos_token, eos_token);

    } catch (const std::exception& e) {
        spdlog::warn("Failed to parse config.json: {}", e.what());
        return std::nullopt;
    }
}

ChatTemplateRenderer ChatTemplateRenderer::fromString(
    const std::string& template_source,
    const std::string& bos_token,
    const std::string& eos_token) {
    return ChatTemplateRenderer(template_source, bos_token, eos_token);
}

std::string ChatTemplateRenderer::render(
    const std::vector<ChatMessage>& messages,
    bool add_generation_prompt) const {

    try {
        // Parse the template
        auto template_root = minja::Parser::parse(template_source_, {
            /* .trim_blocks = */ true,
            /* .lstrip_blocks = */ true,
            /* .keep_trailing_newline = */ false,
        });

        // Convert messages to JSON array
        json messages_json = json::array();
        for (const auto& msg : messages) {
            messages_json.push_back({
                {"role", msg.role},
                {"content", msg.content}
            });
        }

        // Build the context using minja::Context::make
        auto context = minja::Context::make(json({
            {"messages", messages_json},
            {"add_generation_prompt", add_generation_prompt},
        }));
        context->set("bos_token", bos_token_);
        context->set("eos_token", eos_token_);

        // Render the template
        auto result = template_root->render(context);
        return result;

    } catch (const std::exception& e) {
        spdlog::error("Failed to render chat template: {}", e.what());
        throw std::runtime_error(std::string("Chat template rendering failed: ") + e.what());
    }
}

}  // namespace allm
