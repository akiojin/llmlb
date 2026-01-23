/**
 * @file glm.cpp
 * @brief GLM-4.7 architecture implementation
 *
 * Implementation of GLM-4.7 series models (GLM-4.7, GLM-4.7-Flash, GLM-4.7-FP8)
 * for safetensors.cpp.
 */

#include "glm.h"
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>
#include <regex>
#include <stdexcept>

namespace safetensors {
namespace glm {

using json = nlohmann::json;

bool is_glm_architecture(const std::string& config_json) {
    try {
        auto config = json::parse(config_json);

        // Check architectures field
        if (config.contains("architectures") && config["architectures"].is_array()) {
            for (const auto& arch : config["architectures"]) {
                if (arch.is_string()) {
                    std::string arch_str = arch.get<std::string>();
                    if (arch_str.find("GLM") != std::string::npos ||
                        arch_str.find("Glm") != std::string::npos ||
                        arch_str.find("ChatGLM") != std::string::npos) {
                        return true;
                    }
                }
            }
        }

        // Check model_type field
        if (config.contains("model_type") && config["model_type"].is_string()) {
            std::string model_type = config["model_type"].get<std::string>();
            if (model_type.find("glm") != std::string::npos ||
                model_type.find("chatglm") != std::string::npos) {
                return true;
            }
        }

        return false;
    } catch (const json::parse_error& e) {
        spdlog::error("GLM: Failed to parse config.json: {}", e.what());
        return false;
    }
}

GLMConfig parse_glm_config(const std::string& config_json) {
    GLMConfig config;

    try {
        auto j = json::parse(config_json);

        // Model dimensions
        if (j.contains("hidden_size")) {
            config.hidden_size = j["hidden_size"].get<int>();
        }
        if (j.contains("num_hidden_layers")) {
            config.num_hidden_layers = j["num_hidden_layers"].get<int>();
        }
        if (j.contains("vocab_size")) {
            config.vocab_size = j["vocab_size"].get<int>();
        }
        if (j.contains("intermediate_size")) {
            config.intermediate_size = j["intermediate_size"].get<int>();
        }

        // Attention config
        if (j.contains("num_attention_heads")) {
            config.num_attention_heads = j["num_attention_heads"].get<int>();
        }
        if (j.contains("num_key_value_heads")) {
            config.num_key_value_heads = j["num_key_value_heads"].get<int>();
        } else {
            // Default to MHA if not specified
            config.num_key_value_heads = config.num_attention_heads;
        }

        // Calculate head_dim
        if (config.num_attention_heads > 0) {
            config.head_dim = config.hidden_size / config.num_attention_heads;
        }

        // Context length
        if (j.contains("max_position_embeddings")) {
            config.max_position_embeddings = j["max_position_embeddings"].get<int>();
        } else if (j.contains("seq_length")) {
            config.max_position_embeddings = j["seq_length"].get<int>();
        }

        // RoPE config
        if (j.contains("rope_theta")) {
            config.rope_theta = j["rope_theta"].get<float>();
        } else if (j.contains("rotary_emb_base")) {
            config.rope_theta = j["rotary_emb_base"].get<float>();
        }

        // Layer norm
        if (j.contains("layer_norm_eps") || j.contains("layernorm_epsilon")) {
            config.layer_norm_eps = j.value("layer_norm_eps",
                                            j.value("layernorm_epsilon", 1e-5f));
        }
        if (j.contains("use_rms_norm")) {
            config.use_rms_norm = j["use_rms_norm"].get<bool>();
        }

        // MoE config (for GLM-4.7-Flash)
        if (j.contains("num_experts") || j.contains("moe_num_experts")) {
            config.is_moe = true;
            config.moe_config.num_experts = j.value("num_experts",
                                                    j.value("moe_num_experts", 8));
            config.moe_config.num_experts_per_tok = j.value("num_experts_per_tok",
                                                           j.value("moe_top_k", 2));
            config.moe_config.num_shared_experts = j.value("num_shared_experts", 0);
        }

        // FP8 detection
        if (j.contains("torch_dtype") && j["torch_dtype"].is_string()) {
            std::string dtype = j["torch_dtype"].get<std::string>();
            if (dtype.find("float8") != std::string::npos ||
                dtype.find("fp8") != std::string::npos) {
                config.is_fp8 = true;
            }
        }
        if (j.contains("quantization_config")) {
            auto qconfig = j["quantization_config"];
            if (qconfig.contains("quant_method") && qconfig["quant_method"].is_string()) {
                std::string method = qconfig["quant_method"].get<std::string>();
                if (method.find("fp8") != std::string::npos) {
                    config.is_fp8 = true;
                }
            }
        }

        // Special tokens
        if (j.contains("bos_token_id")) {
            config.bos_token_id = j["bos_token_id"].get<int>();
        }
        if (j.contains("eos_token_id")) {
            if (j["eos_token_id"].is_array()) {
                // Some models have multiple EOS tokens
                config.eos_token_id = j["eos_token_id"][0].get<int>();
            } else {
                config.eos_token_id = j["eos_token_id"].get<int>();
            }
        }
        if (j.contains("pad_token_id")) {
            config.pad_token_id = j["pad_token_id"].get<int>();
        }

        spdlog::info("GLM config parsed: hidden_size={}, layers={}, vocab={}, "
                     "heads={}, kv_heads={}, is_moe={}, is_fp8={}",
                     config.hidden_size, config.num_hidden_layers, config.vocab_size,
                     config.num_attention_heads, config.num_key_value_heads,
                     config.is_moe, config.is_fp8);

    } catch (const json::exception& e) {
        spdlog::error("GLM: Failed to parse config.json: {}", e.what());
        throw std::runtime_error("GLM config parsing failed: " + std::string(e.what()));
    }

    return config;
}

int extract_layer_index(const std::string& tensor_name) {
    // Pattern: "transformer.encoder.layers.N.*" or "model.layers.N.*"
    std::regex layer_pattern(R"((?:transformer\.encoder\.layers|model\.layers)\.(\d+)\.)");
    std::smatch match;

    if (std::regex_search(tensor_name, match, layer_pattern)) {
        return std::stoi(match[1].str());
    }

    return -1;
}

GLMWeights load_glm_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const GLMConfig& config) {

    GLMWeights weights;
    weights.layers.resize(config.num_hidden_layers);

    // TODO: Implement weight loading from safetensors
    // This requires mapping GLM tensor names to our weight structure

    spdlog::warn("GLM weight loading not yet fully implemented");

    return weights;
}

struct ggml_tensor* glm_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input_ids,
    const GLMWeights& weights,
    const GLMConfig& config,
    void* kv_cache) {

    // TODO: Implement forward pass
    spdlog::warn("GLM forward pass not yet implemented");
    return nullptr;
}

struct ggml_tensor* glm_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    int layer_idx,
    const GLMLayerWeights& weights,
    const GLMConfig& config,
    void* kv_cache) {

    // TODO: Implement single layer forward pass
    spdlog::warn("GLM layer forward pass not yet implemented");
    return nullptr;
}

std::vector<ThinkingBlock> parse_thinking_blocks(
    const std::string& text,
    const std::string& thinking_start,
    const std::string& thinking_end) {

    std::vector<ThinkingBlock> blocks;
    size_t pos = 0;

    while (pos < text.length()) {
        size_t start = text.find(thinking_start, pos);
        if (start == std::string::npos) {
            break;
        }

        size_t content_start = start + thinking_start.length();
        size_t end = text.find(thinking_end, content_start);
        if (end == std::string::npos) {
            // Unclosed thinking block - treat rest as thinking
            ThinkingBlock block;
            block.content = text.substr(content_start);
            block.start_pos = start;
            block.end_pos = text.length();
            blocks.push_back(block);
            break;
        }

        ThinkingBlock block;
        block.content = text.substr(content_start, end - content_start);
        block.start_pos = start;
        block.end_pos = end + thinking_end.length();
        blocks.push_back(block);

        pos = block.end_pos;
    }

    return blocks;
}

std::string remove_thinking_blocks(
    const std::string& text,
    const std::string& thinking_start,
    const std::string& thinking_end) {

    std::string result;
    size_t pos = 0;

    while (pos < text.length()) {
        size_t start = text.find(thinking_start, pos);
        if (start == std::string::npos) {
            result += text.substr(pos);
            break;
        }

        // Add text before thinking block
        result += text.substr(pos, start - pos);

        size_t content_start = start + thinking_start.length();
        size_t end = text.find(thinking_end, content_start);
        if (end == std::string::npos) {
            // Unclosed thinking block - remove rest
            break;
        }

        pos = end + thinking_end.length();
    }

    return result;
}

} // namespace glm
} // namespace safetensors
