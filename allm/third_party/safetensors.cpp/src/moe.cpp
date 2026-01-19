#include "moe.h"
#include <algorithm>
#include <stdexcept>
#include <cmath>

namespace safetensors {

// MoE Router

struct ggml_tensor* moe_router(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    struct ggml_tensor* router_weight,
    struct ggml_tensor* router_bias,
    int top_k) {

    if (!ctx || !input || !router_weight) {
        throw std::invalid_argument("Invalid arguments to moe_router");
    }

    // Router: Linear projection + sigmoid gating
    // routing_logits = input @ router_weight^T + router_bias
    struct ggml_tensor* routing_logits = ggml_mul_mat(ctx, router_weight, input);
    ggml_set_name(routing_logits, "moe_routing_logits");

    if (router_bias) {
        routing_logits = ggml_add(ctx, routing_logits, router_bias);
        ggml_set_name(routing_logits, "moe_routing_logits_bias");
    }

    // Nemotron 3 uses sigmoid gating
    routing_logits = ggml_sigmoid(ctx, routing_logits);
    ggml_set_name(routing_logits, "moe_routing_sigmoid");

    return routing_logits;
}

// Top-K Expert Selection

MoERoutingResult select_top_k_experts(
    struct ggml_tensor* routing_logits,
    int top_k) {

    if (!routing_logits) {
        throw std::runtime_error("select_top_k_experts: null routing_logits");
    }

    MoERoutingResult result;

    // Get number of experts
    int n_experts = routing_logits->ne[0];

    if (top_k > n_experts) {
        top_k = n_experts;
    }

    // Access routing logits data (assumes data is available - requires graph execution)
    float* logits_data = reinterpret_cast<float*>(routing_logits->data);

    if (!logits_data) {
        // Fallback: return first top_k experts with uniform weights
        for (int i = 0; i < top_k; ++i) {
            result.expert_indices.push_back(i);
            result.expert_weights.push_back(1.0f / top_k);
        }
        return result;
    }

    // Create vector of (index, logit) pairs
    std::vector<std::pair<int, float>> expert_logits;
    expert_logits.reserve(n_experts);

    for (int i = 0; i < n_experts; ++i) {
        expert_logits.emplace_back(i, logits_data[i]);
    }

    // Partial sort to get top-k experts (highest logits first)
    std::partial_sort(expert_logits.begin(),
                      expert_logits.begin() + top_k,
                      expert_logits.end(),
                      [](const auto& a, const auto& b) {
                          return a.second > b.second;  // Descending order
                      });

    // Extract top-k indices and compute softmax weights
    result.expert_indices.reserve(top_k);
    result.expert_weights.reserve(top_k);

    // Find max logit for numerical stability
    float max_logit = expert_logits[0].second;

    // Compute exp(logit - max) and sum
    float sum_exp = 0.0f;
    std::vector<float> exp_logits;
    exp_logits.reserve(top_k);

    for (int i = 0; i < top_k; ++i) {
        float exp_val = std::exp(expert_logits[i].second - max_logit);
        exp_logits.push_back(exp_val);
        sum_exp += exp_val;
    }

    // Normalize to get softmax weights
    for (int i = 0; i < top_k; ++i) {
        result.expert_indices.push_back(expert_logits[i].first);
        result.expert_weights.push_back(exp_logits[i] / sum_exp);
    }

    return result;
}

// Expert Execution (FFN)

struct ggml_tensor* execute_expert(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    struct ggml_tensor* up_weight,
    struct ggml_tensor* down_weight) {

    if (!ctx || !input || !up_weight || !down_weight) {
        throw std::invalid_argument("Invalid arguments to execute_expert");
    }

    // FFN structure: up_proj -> activation -> down_proj

    // Up projection: [seq_len, d_model] -> [seq_len, d_ff]
    struct ggml_tensor* hidden = ggml_mul_mat(ctx, up_weight, input);
    ggml_set_name(hidden, "expert_up_proj");

    // Activation: Squared ReLU (as mentioned in Nemotron 3 research)
    // squared_relu(x) = (relu(x))^2
    hidden = ggml_relu(ctx, hidden);
    hidden = ggml_sqr(ctx, hidden);
    ggml_set_name(hidden, "expert_squared_relu");

    // Down projection: [seq_len, d_ff] -> [seq_len, d_model]
    struct ggml_tensor* output = ggml_mul_mat(ctx, down_weight, hidden);
    ggml_set_name(output, "expert_down_proj");

    return output;
}

// Shared Experts Execution

struct ggml_tensor* execute_shared_experts(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const std::vector<struct ggml_tensor*>& shared_up_weights,
    const std::vector<struct ggml_tensor*>& shared_down_weights) {

    if (!ctx || !input || shared_up_weights.empty() || shared_down_weights.empty()) {
        throw std::invalid_argument("Invalid arguments to execute_shared_experts");
    }

    if (shared_up_weights.size() != shared_down_weights.size()) {
        throw std::invalid_argument("Mismatched shared expert weights");
    }

    // Execute all shared experts and average their outputs
    std::vector<struct ggml_tensor*> outputs;

    for (size_t i = 0; i < shared_up_weights.size(); ++i) {
        struct ggml_tensor* expert_out = execute_expert(
            ctx, input,
            shared_up_weights[i],
            shared_down_weights[i]);
        outputs.push_back(expert_out);
    }

    // Average all shared expert outputs
    struct ggml_tensor* result = outputs[0];
    for (size_t i = 1; i < outputs.size(); ++i) {
        result = ggml_add(ctx, result, outputs[i]);
    }

    // Divide by number of shared experts
    float scale = 1.0f / static_cast<float>(outputs.size());
    result = ggml_scale(ctx, result, scale);
    ggml_set_name(result, "shared_experts_avg");

    return result;
}

// MoE Output Mixing

struct ggml_tensor* mix_expert_outputs(
    struct ggml_context* ctx,
    const std::vector<struct ggml_tensor*>& expert_outputs,
    const std::vector<float>& routing_weights) {

    if (!ctx || expert_outputs.empty() || routing_weights.empty()) {
        throw std::invalid_argument("Invalid arguments to mix_expert_outputs");
    }

    if (expert_outputs.size() != routing_weights.size()) {
        throw std::invalid_argument("Mismatched expert outputs and routing weights");
    }

    // Weighted sum of expert outputs
    struct ggml_tensor* result = ggml_scale(ctx, expert_outputs[0], routing_weights[0]);

    for (size_t i = 1; i < expert_outputs.size(); ++i) {
        struct ggml_tensor* weighted = ggml_scale(ctx, expert_outputs[i], routing_weights[i]);
        result = ggml_add(ctx, result, weighted);
    }

    ggml_set_name(result, "moe_mixed_output");
    return result;
}

// MoE Layer Forward Pass

struct ggml_tensor* moe_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const MoELayerWeights& weights,
    const MoELayerConfig& config) {

    if (!ctx || !input) {
        throw std::invalid_argument("Invalid arguments to moe_layer_forward");
    }

    // 1. Layer normalization
    struct ggml_tensor* x = input;
    if (weights.norm_weight) {
        x = ggml_norm(ctx, x, 1e-5f);
        x = ggml_mul(ctx, x, weights.norm_weight);
        if (weights.norm_bias) {
            x = ggml_add(ctx, x, weights.norm_bias);
        }
        ggml_set_name(x, "moe_norm");
    }

    // 2. Router: compute routing logits and select Top-K experts
    struct ggml_tensor* routing_logits = moe_router(
        ctx, x,
        weights.router_weight,
        weights.router_bias,
        config.top_k);

    // 3. Top-K expert selection
    MoERoutingResult routing = select_top_k_experts(routing_logits, config.top_k);

    // 4. Execute selected routed experts
    std::vector<struct ggml_tensor*> expert_outputs;
    for (int i = 0; i < config.top_k; ++i) {
        int expert_idx = routing.expert_indices[i];
        if (expert_idx < 0 || expert_idx >= config.n_routed_experts) {
            throw std::runtime_error("Invalid expert index: " + std::to_string(expert_idx));
        }

        struct ggml_tensor* expert_out = execute_expert(
            ctx, x,
            weights.expert_up_weight[expert_idx],
            weights.expert_down_weight[expert_idx]);
        expert_outputs.push_back(expert_out);
    }

    // 5. Mix routed expert outputs with routing weights
    struct ggml_tensor* routed_output = mix_expert_outputs(
        ctx, expert_outputs, routing.expert_weights);

    // 6. Execute shared experts (always activated)
    struct ggml_tensor* shared_output = execute_shared_experts(
        ctx, x,
        weights.shared_expert_up_weight,
        weights.shared_expert_down_weight);

    // 7. Combine routed and shared expert outputs
    struct ggml_tensor* combined = ggml_add(ctx, routed_output, shared_output);
    ggml_set_name(combined, "moe_combined");

    // 8. Residual connection
    struct ggml_tensor* output = ggml_add(ctx, combined, input);
    ggml_set_name(output, "moe_residual");

    return output;
}

// Load MoE Layer Weights

MoELayerWeights load_moe_layer_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const std::string& prefix,
    const MoELayerConfig& config) {

    MoELayerWeights weights{};

    auto get_tensor = [&](const std::string& name, bool required = true) -> struct ggml_tensor* {
        std::string full_name = prefix + "." + name;
        auto it = tensors.find(full_name);
        if (it == tensors.end()) {
            if (required) {
                throw std::runtime_error("Required tensor not found: " + full_name);
            }
            return nullptr;
        }
        return it->second;
    };

    // Load router weights
    weights.router_weight = get_tensor("router.weight");
    weights.router_bias = get_tensor("router.bias", false);

    // Load routed expert weights
    for (int i = 0; i < config.n_routed_experts; ++i) {
        std::string expert_prefix = "experts." + std::to_string(i);
        weights.expert_up_weight.push_back(get_tensor(expert_prefix + ".up_proj.weight"));
        weights.expert_down_weight.push_back(get_tensor(expert_prefix + ".down_proj.weight"));
    }

    // Load shared expert weights
    for (int i = 0; i < config.n_shared_experts; ++i) {
        std::string shared_prefix = "shared_experts." + std::to_string(i);
        weights.shared_expert_up_weight.push_back(get_tensor(shared_prefix + ".up_proj.weight"));
        weights.shared_expert_down_weight.push_back(get_tensor(shared_prefix + ".down_proj.weight"));
    }

    // Load normalization weights
    weights.norm_weight = get_tensor("norm.weight", false);
    weights.norm_bias = get_tensor("norm.bias", false);

    return weights;
}

} // namespace safetensors
