#pragma once

#include <filesystem>
#include <map>
#include <optional>
#include <string>
#include <vector>

#include "core/engine_types.h"

namespace xllm {

struct Modelfile {
    std::string base_model;
    std::string system_prompt;
    std::string template_text;
    std::vector<ChatMessage> messages;
    std::map<std::string, std::string> parameters;
    std::string raw_text;

    static std::filesystem::path pathForModel(const std::string& model_name);
    static std::optional<Modelfile> loadForModel(const std::string& model_name, std::string& error);
};

}  // namespace xllm
