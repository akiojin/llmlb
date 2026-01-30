#include "models/modelfile.h"

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <fstream>
#include <sstream>

#include "models/model_storage.h"

namespace fs = std::filesystem;

namespace xllm {

namespace {

std::string trim(std::string value) {
    auto is_space = [](unsigned char c) { return std::isspace(c) != 0; };
    value.erase(value.begin(), std::find_if(value.begin(), value.end(), [&](unsigned char c) { return !is_space(c); }));
    value.erase(std::find_if(value.rbegin(), value.rend(), [&](unsigned char c) { return !is_space(c); }).base(), value.end());
    return value;
}

std::string to_lower(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}

bool starts_with(const std::string& value, const std::string& prefix) {
    return value.size() >= prefix.size() && value.compare(0, prefix.size(), prefix) == 0;
}

std::string strip_quotes(const std::string& value) {
    if (value.size() >= 2) {
        char first = value.front();
        char last = value.back();
        if ((first == '"' && last == '"') || (first == '\'' && last == '\'')) {
            return value.substr(1, value.size() - 2);
        }
    }
    return value;
}

bool parse_block_value(const std::vector<std::string>& lines,
                       size_t& index,
                       const std::string& rest,
                       std::string& out,
                       std::string& error) {
    std::string value = trim(rest);
    if (starts_with(value, "\"\"\"")) {
        std::string current = value.substr(3);
        size_t end = current.find("\"\"\"");
        if (end != std::string::npos) {
            out = current.substr(0, end);
            return true;
        }
        std::ostringstream oss;
        oss << current;
        while (++index < lines.size()) {
            std::string line = lines[index];
            size_t end_pos = line.find("\"\"\"");
            if (end_pos != std::string::npos) {
                if (oss.tellp() > 0) oss << "\n";
                oss << line.substr(0, end_pos);
                out = oss.str();
                return true;
            }
            if (oss.tellp() > 0) oss << "\n";
            oss << line;
        }
        error = "Unterminated block string (\"\"\")";
        return false;
    }

    out = strip_quotes(value);
    return true;
}

}  // namespace

fs::path Modelfile::pathForModel(const std::string& model_name) {
    const char* home = std::getenv("HOME");
    fs::path base = home ? fs::path(home) : fs::path(".");
    const std::string dir = ModelStorage::modelNameToDir(model_name);
    return base / ".xllm" / "Modelfiles" / dir / "Modelfile";
}

std::optional<Modelfile> Modelfile::loadForModel(const std::string& model_name, std::string& error) {
    const fs::path path = Modelfile::pathForModel(model_name);
    if (!fs::exists(path)) {
        return std::nullopt;
    }

    std::ifstream file(path);
    if (!file.is_open()) {
        error = "Failed to open Modelfile: " + path.string();
        return std::nullopt;
    }

    std::ostringstream raw;
    raw << file.rdbuf();
    std::string raw_text = raw.str();

    file.clear();
    file.seekg(0, std::ios::beg);

    std::vector<std::string> lines;
    std::string line;
    while (std::getline(file, line)) {
        lines.push_back(line);
    }

    Modelfile modelfile;
    modelfile.raw_text = raw_text;

    for (size_t i = 0; i < lines.size(); ++i) {
        std::string trimmed = trim(lines[i]);
        if (trimmed.empty() || starts_with(trimmed, "#")) {
            continue;
        }

        if (starts_with(trimmed, "FROM ")) {
            modelfile.base_model = trim(trimmed.substr(5));
            continue;
        }

        if (starts_with(trimmed, "PARAMETER ")) {
            std::string rest = trim(trimmed.substr(10));
            auto pos = rest.find(' ');
            if (pos == std::string::npos) {
                error = "PARAMETER requires name and value";
                return std::nullopt;
            }
            std::string key = to_lower(trim(rest.substr(0, pos)));
            std::string value;
            if (!parse_block_value(lines, i, rest.substr(pos + 1), value, error)) {
                return std::nullopt;
            }
            modelfile.parameters[key] = value;
            continue;
        }

        if (starts_with(trimmed, "SYSTEM")) {
            std::string rest = trim(trimmed.substr(6));
            if (!parse_block_value(lines, i, rest, modelfile.system_prompt, error)) {
                return std::nullopt;
            }
            continue;
        }

        if (starts_with(trimmed, "TEMPLATE")) {
            std::string rest = trim(trimmed.substr(8));
            if (!parse_block_value(lines, i, rest, modelfile.template_text, error)) {
                return std::nullopt;
            }
            continue;
        }

        if (starts_with(trimmed, "MESSAGE")) {
            std::string rest = trim(trimmed.substr(7));
            auto pos = rest.find(' ');
            if (pos == std::string::npos) {
                error = "MESSAGE requires role and content";
                return std::nullopt;
            }
            std::string role = trim(rest.substr(0, pos));
            std::string content;
            if (!parse_block_value(lines, i, rest.substr(pos + 1), content, error)) {
                return std::nullopt;
            }
            modelfile.messages.push_back(ChatMessage{role, content});
            continue;
        }

        error = "Unknown Modelfile directive: " + trimmed;
        return std::nullopt;
    }

    return modelfile;
}

}  // namespace xllm
