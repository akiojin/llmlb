/**
 * @file chat_template.cpp
 * @brief Jinja2-subset chat template implementation
 *
 * Supports a limited subset of Jinja2:
 * - {{ variable }} - variable interpolation
 * - {% for item in items %} ... {% endfor %} - for loops
 * - {% if condition %} ... {% elif %} ... {% else %} ... {% endif %} - conditionals
 * - {{ loop.index }} - loop index (1-based)
 * - {{ loop.index0 }} - loop index (0-based)
 */

#include "safetensors_internal.h"
#include <sstream>
#include <regex>

namespace stcpp {

/* Template node types */
enum class NodeType {
    TEXT,
    VARIABLE,
    FOR_LOOP,
    IF_BLOCK,
    ELIF_BLOCK,
    ELSE_BLOCK,
    END_FOR,
    END_IF
};

struct TemplateNode {
    NodeType type;
    std::string content;
    std::string var_name;       // For loops: "message"
    std::string iterable;       // For loops: "messages"
    std::string condition;      // For conditionals
    std::vector<TemplateNode> children;
};

/* Parse template into nodes */
static bool tokenize_template(
    const std::string& tmpl,
    std::vector<TemplateNode>& nodes,
    std::string& error
) {
    fprintf(stderr, "[DEBUG] tokenize_template: entered, len=%zu\n", tmpl.size());
    fflush(stderr);

    size_t pos = 0;
    size_t len = tmpl.size();
    int iteration = 0;

    while (pos < len) {
        iteration++;
        if (iteration % 1000 == 0) {
            fprintf(stderr, "[DEBUG] tokenize_template: iteration=%d, pos=%zu/%zu\n", iteration, pos, len);
            fflush(stderr);
        }
        // Find next template tag
        size_t tag_start = tmpl.find("{", pos);

        if (tag_start == std::string::npos) {
            // Rest is plain text
            if (pos < len) {
                TemplateNode node;
                node.type = NodeType::TEXT;
                node.content = tmpl.substr(pos);
                nodes.push_back(node);
            }
            break;
        }

        // Add text before tag
        if (tag_start > pos) {
            TemplateNode node;
            node.type = NodeType::TEXT;
            node.content = tmpl.substr(pos, tag_start - pos);
            nodes.push_back(node);
        }

        // Check tag type
        if (tag_start + 1 < len && tmpl[tag_start + 1] == '{') {
            // Variable: {{ ... }}
            size_t var_end = tmpl.find("}}", tag_start + 2);
            if (var_end == std::string::npos) {
                error = "Unclosed variable tag";
                return false;
            }

            TemplateNode node;
            node.type = NodeType::VARIABLE;
            node.content = tmpl.substr(tag_start + 2, var_end - tag_start - 2);
            // Trim whitespace
            size_t start = node.content.find_first_not_of(" \t");
            size_t end = node.content.find_last_not_of(" \t");
            if (start != std::string::npos && end != std::string::npos) {
                node.content = node.content.substr(start, end - start + 1);
            }
            nodes.push_back(node);
            pos = var_end + 2;

        } else if (tag_start + 1 < len && tmpl[tag_start + 1] == '%') {
            // Control: {% ... %}
            size_t ctrl_end = tmpl.find("%}", tag_start + 2);
            if (ctrl_end == std::string::npos) {
                error = "Unclosed control tag";
                return false;
            }

            std::string ctrl = tmpl.substr(tag_start + 2, ctrl_end - tag_start - 2);
            // Trim whitespace
            size_t start = ctrl.find_first_not_of(" \t");
            size_t end = ctrl.find_last_not_of(" \t");
            if (start != std::string::npos && end != std::string::npos) {
                ctrl = ctrl.substr(start, end - start + 1);
            }

            TemplateNode node;

            if (ctrl.substr(0, 3) == "for") {
                // {% for x in y %}
                node.type = NodeType::FOR_LOOP;
                // Parse: "for message in messages"
                fprintf(stderr, "[DEBUG] tokenize_template: parsing for loop: '%s'\n", ctrl.c_str());
                fflush(stderr);
                std::regex for_regex(R"(for\s+(\w+)\s+in\s+(\w+))");
                std::smatch match;
                fprintf(stderr, "[DEBUG] tokenize_template: regex_search starting\n");
                fflush(stderr);
                if (std::regex_search(ctrl, match, for_regex)) {
                    node.var_name = match[1].str();
                    node.iterable = match[2].str();
                    fprintf(stderr, "[DEBUG] tokenize_template: for loop: var=%s iterable=%s\n",
                            node.var_name.c_str(), node.iterable.c_str());
                    fflush(stderr);
                }
            } else if (ctrl == "endfor") {
                node.type = NodeType::END_FOR;
            } else if (ctrl.substr(0, 2) == "if") {
                node.type = NodeType::IF_BLOCK;
                node.condition = ctrl.substr(2);
                // Trim
                start = node.condition.find_first_not_of(" \t");
                if (start != std::string::npos) {
                    node.condition = node.condition.substr(start);
                }
            } else if (ctrl.substr(0, 4) == "elif") {
                node.type = NodeType::ELIF_BLOCK;
                node.condition = ctrl.substr(4);
                start = node.condition.find_first_not_of(" \t");
                if (start != std::string::npos) {
                    node.condition = node.condition.substr(start);
                }
            } else if (ctrl == "else") {
                node.type = NodeType::ELSE_BLOCK;
            } else if (ctrl == "endif") {
                node.type = NodeType::END_IF;
            }

            nodes.push_back(node);
            pos = ctrl_end + 2;
        } else {
            // Not a template tag, just a brace
            pos = tag_start + 1;
        }
    }

    return true;
}

/* Evaluate a simple condition */
static bool eval_condition(
    const std::string& condition,
    const ChatMessage& message,
    bool add_generation_prompt
) {
    // Support: message.role == 'user', add_generation_prompt, etc.
    if (condition.find("message.role") != std::string::npos) {
        // Extract comparison value
        std::regex cmp_regex(R"(message\.role\s*==\s*['\"](\w+)['\"])");
        std::smatch match;
        if (std::regex_search(condition, match, cmp_regex)) {
            return message.role == match[1].str();
        }
    }

    if (condition.find("add_generation_prompt") != std::string::npos) {
        return add_generation_prompt;
    }

    // Default: treat as truthy
    return true;
}

/* Global cache for parsed templates */
static std::vector<TemplateNode> g_cached_nodes;
static std::string g_cached_template;

/* Parse chat template */
bool parse_chat_template(
    const std::string& template_str,
    ChatTemplate& tmpl,
    std::string& error
) {
    fprintf(stderr, "[DEBUG] parse_chat_template: entered, template_len=%zu\n", template_str.size());
    fflush(stderr);

    tmpl.raw_template = template_str;
    tmpl.valid = false;

    // Check cache
    if (g_cached_template == template_str && !g_cached_nodes.empty()) {
        fprintf(stderr, "[DEBUG] parse_chat_template: using cached nodes (%zu)\n", g_cached_nodes.size());
        fflush(stderr);
        tmpl.valid = true;
        return true;
    }

    std::vector<TemplateNode> nodes;
    fprintf(stderr, "[DEBUG] parse_chat_template: calling tokenize_template\n");
    fflush(stderr);
    if (!tokenize_template(template_str, nodes, error)) {
        fprintf(stderr, "[DEBUG] parse_chat_template: tokenize_template failed: %s\n", error.c_str());
        fflush(stderr);
        return false;
    }
    fprintf(stderr, "[DEBUG] parse_chat_template: tokenize_template succeeded, nodes=%zu\n", nodes.size());
    fflush(stderr);

    // Basic validation: check for matching blocks
    int for_depth = 0;
    int if_depth = 0;
    for (const auto& node : nodes) {
        switch (node.type) {
            case NodeType::FOR_LOOP: for_depth++; break;
            case NodeType::END_FOR: for_depth--; break;
            case NodeType::IF_BLOCK: if_depth++; break;
            case NodeType::END_IF: if_depth--; break;
            default: break;
        }
        if (for_depth < 0 || if_depth < 0) {
            error = "Mismatched template blocks";
            return false;
        }
    }

    if (for_depth != 0) {
        error = "Unclosed for loop";
        return false;
    }
    if (if_depth != 0) {
        error = "Unclosed if block";
        return false;
    }

    // Cache the parsed nodes
    g_cached_template = template_str;
    g_cached_nodes = std::move(nodes);

    tmpl.valid = true;
    return true;
}

/* Apply chat template to messages */
bool apply_chat_template(
    const ChatTemplate& tmpl,
    const std::vector<ChatMessage>& messages,
    std::string& result,
    std::string& error,
    bool add_generation_prompt
) {
    if (!tmpl.valid) {
        error = "Invalid template";
        return false;
    }

    result.clear();

    // ChatML format detection: if template contains <|im_start|>, use ChatML
    // This handles complex Qwen/Llama templates that our parser can't process
    if (tmpl.raw_template.find("<|im_start|>") != std::string::npos) {
        fprintf(stderr, "[DEBUG] apply_chat_template: ChatML format detected, using fallback\n");
        fflush(stderr);
        std::stringstream ss;

        // Check if first message is system, otherwise use default
        bool has_system = !messages.empty() && messages[0].role == "system";
        size_t start_idx = 0;

        if (has_system) {
            ss << "<|im_start|>system\n" << messages[0].content << "<|im_end|>\n";
            start_idx = 1;
        } else {
            ss << "<|im_start|>system\nYou are a helpful assistant.<|im_end|>\n";
        }

        for (size_t i = start_idx; i < messages.size(); i++) {
            ss << "<|im_start|>" << messages[i].role << "\n";
            ss << messages[i].content << "<|im_end|>\n";
        }

        if (add_generation_prompt) {
            ss << "<|im_start|>assistant\n";
        }

        result = ss.str();
        fprintf(stderr, "[DEBUG] apply_chat_template: ChatML result_len=%zu\n", result.size());
        fflush(stderr);
        return true;
    }

    // Use cached nodes if available
    std::vector<TemplateNode> nodes;
    if (g_cached_template == tmpl.raw_template && !g_cached_nodes.empty()) {
        fprintf(stderr, "[DEBUG] apply_chat_template: using cached nodes (%zu)\n", g_cached_nodes.size());
        fflush(stderr);
        nodes = g_cached_nodes;
    } else {
        fprintf(stderr, "[DEBUG] apply_chat_template: re-tokenizing template\n");
        fflush(stderr);
        if (!tokenize_template(tmpl.raw_template, nodes, error)) {
            return false;
        }
    }

    // Process nodes with a simple interpreter
    std::function<bool(const std::vector<TemplateNode>&, size_t&, std::stringstream&,
                       const ChatMessage*, int)> process;

    int debug_call_count = 0;

    process = [&](const std::vector<TemplateNode>& nodes, size_t& idx,
                  std::stringstream& out, const ChatMessage* current_msg, int loop_idx) -> bool {
        while (idx < nodes.size()) {
            debug_call_count++;
            if (debug_call_count % 100 == 0) {
                fprintf(stderr, "[DEBUG] process: call=%d, idx=%zu/%zu, type=%d\n",
                        debug_call_count, idx, nodes.size(), static_cast<int>(nodes[idx].type));
                fflush(stderr);
            }
            if (debug_call_count > 10000) {
                fprintf(stderr, "[DEBUG] process: ABORT - too many iterations (%d)\n", debug_call_count);
                fflush(stderr);
                return false;
            }

            const auto& node = nodes[idx];

            switch (node.type) {
                case NodeType::TEXT:
                    out << node.content;
                    idx++;
                    break;

                case NodeType::VARIABLE: {
                    std::string var = node.content;
                    if (var == "message.role" && current_msg) {
                        out << current_msg->role;
                    } else if (var == "message.content" && current_msg) {
                        out << current_msg->content;
                    } else if (var == "loop.index") {
                        out << (loop_idx + 1);
                    } else if (var == "loop.index0") {
                        out << loop_idx;
                    }
                    idx++;
                    break;
                }

                case NodeType::FOR_LOOP: {
                    if (node.iterable == "messages") {
                        idx++;  // Move past FOR_LOOP
                        size_t loop_start = idx;

                        for (size_t mi = 0; mi < messages.size(); mi++) {
                            idx = loop_start;
                            // Find endfor
                            int depth = 1;
                            while (idx < nodes.size() && depth > 0) {
                                if (nodes[idx].type == NodeType::FOR_LOOP) depth++;
                                if (nodes[idx].type == NodeType::END_FOR) {
                                    depth--;
                                    if (depth == 0) break;
                                }

                                if (!process(nodes, idx, out, &messages[mi], static_cast<int>(mi))) {
                                    return false;
                                }
                            }
                        }
                        // Skip past endfor
                        if (idx < nodes.size() && nodes[idx].type == NodeType::END_FOR) {
                            idx++;
                        }
                    } else {
                        idx++;
                    }
                    break;
                }

                case NodeType::IF_BLOCK: {
                    bool condition_met = false;
                    if (current_msg) {
                        condition_met = eval_condition(node.condition, *current_msg, add_generation_prompt);
                    } else if (node.condition.find("add_generation_prompt") != std::string::npos) {
                        condition_met = add_generation_prompt;
                    }

                    idx++;

                    if (condition_met) {
                        // Process until elif/else/endif
                        while (idx < nodes.size()) {
                            if (nodes[idx].type == NodeType::ELIF_BLOCK ||
                                nodes[idx].type == NodeType::ELSE_BLOCK ||
                                nodes[idx].type == NodeType::END_IF) {
                                break;
                            }
                            if (!process(nodes, idx, out, current_msg, loop_idx)) {
                                return false;
                            }
                        }
                        // Skip to endif
                        int depth = 1;
                        while (idx < nodes.size() && depth > 0) {
                            if (nodes[idx].type == NodeType::IF_BLOCK) depth++;
                            if (nodes[idx].type == NodeType::END_IF) depth--;
                            idx++;
                        }
                    } else {
                        // Skip to elif/else/endif
                        while (idx < nodes.size()) {
                            if (nodes[idx].type == NodeType::ELIF_BLOCK) {
                                // Process elif
                                idx++;
                                break;
                            }
                            if (nodes[idx].type == NodeType::ELSE_BLOCK) {
                                idx++;
                                // Process else block
                                while (idx < nodes.size() && nodes[idx].type != NodeType::END_IF) {
                                    if (!process(nodes, idx, out, current_msg, loop_idx)) {
                                        return false;
                                    }
                                }
                                if (idx < nodes.size()) idx++;  // Skip endif
                                break;
                            }
                            if (nodes[idx].type == NodeType::END_IF) {
                                idx++;
                                break;
                            }
                            idx++;
                        }
                    }
                    break;
                }

                case NodeType::ELIF_BLOCK:
                case NodeType::ELSE_BLOCK:
                case NodeType::END_IF:
                case NodeType::END_FOR:
                    // These are handled by their parent blocks
                    return true;
            }
        }
        return true;
    };

    fprintf(stderr, "[DEBUG] apply_chat_template: starting process, nodes=%zu\n", nodes.size());
    fflush(stderr);

    std::stringstream ss;
    size_t idx = 0;
    if (!process(nodes, idx, ss, nullptr, 0)) {
        fprintf(stderr, "[DEBUG] apply_chat_template: process returned false\n");
        fflush(stderr);
        return false;
    }

    fprintf(stderr, "[DEBUG] apply_chat_template: process done, idx=%zu\n", idx);
    fflush(stderr);

    result = ss.str();

    fprintf(stderr, "[DEBUG] apply_chat_template: returning, result_len=%zu\n", result.size());
    // Print first 100 bytes as hex
    fprintf(stderr, "[DEBUG] apply_chat_template: first 50 bytes hex: ");
    for (size_t i = 0; i < std::min((size_t)50, result.size()); i++) {
        fprintf(stderr, "%02x ", (unsigned char)result[i]);
    }
    fprintf(stderr, "\n");
    fflush(stderr);

    return true;
}

}  // namespace stcpp
