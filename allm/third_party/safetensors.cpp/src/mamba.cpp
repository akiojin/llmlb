#include "mamba.h"
#include <cmath>
#include <stdexcept>
#include <string>

namespace safetensors {

// MambaState implementation

MambaState::MambaState(struct ggml_context* ctx, const MambaLayerConfig& config)
    : state(nullptr), conv_state(nullptr), sequence_length(0) {
    if (!ctx) {
        throw std::invalid_argument("ggml_context is null");
    }

    // Allocate state tensor [d_state, d_inner]
    state = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.d_state, config.d_inner);
    ggml_set_name(state, "mamba_state");
    ggml_set_zero(state);

    // Allocate conv_state tensor [conv_kernel_size, d_inner]
    conv_state = ggml_new_tensor_2d(ctx, GGML_TYPE_F32, config.conv_kernel_size, config.d_inner);
    ggml_set_name(conv_state, "mamba_conv_state");
    ggml_set_zero(conv_state);
}

void MambaState::reset() {
    if (state) {
        ggml_set_zero(state);
    }
    if (conv_state) {
        ggml_set_zero(conv_state);
    }
    sequence_length = 0;
}

// SSM Convolution implementation

struct ggml_tensor* mamba_ssm_conv(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* conv_weight,
    struct ggml_tensor* conv_bias,
    struct ggml_tensor* conv_state) {

    if (!ctx || !x || !conv_weight) {
        throw std::invalid_argument("Invalid arguments to mamba_ssm_conv");
    }

    // Use ggml_ssm_conv operation
    // conv_state is used for inference (maintaining state across tokens)
    struct ggml_tensor* conv_out = ggml_ssm_conv(ctx, x, conv_weight);
    ggml_set_name(conv_out, "mamba_conv_out");

    // Add bias if provided
    if (conv_bias) {
        conv_out = ggml_add(ctx, conv_out, conv_bias);
        ggml_set_name(conv_out, "mamba_conv_out_bias");
    }

    // Apply SiLU (Swish) activation: x * sigmoid(x)
    conv_out = ggml_silu(ctx, conv_out);
    ggml_set_name(conv_out, "mamba_conv_out_silu");

    return conv_out;
}

// SSM Scan implementation

struct ggml_tensor* mamba_ssm_scan(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* dt,
    struct ggml_tensor* A,
    struct ggml_tensor* B,
    struct ggml_tensor* C,
    struct ggml_tensor* state,
    struct ggml_tensor* ids) {

    if (!ctx || !x || !dt || !A || !B || !C) {
        throw std::invalid_argument("Invalid arguments to mamba_ssm_scan");
    }

    // Use ggml_ssm_scan operation
    // This performs the state space model scan:
    // s_t = A * s_{t-1} + B * x_t
    // y_t = C * s_t
    struct ggml_tensor* scan_out = ggml_ssm_scan(ctx, state, x, dt, A, B, C, ids);
    ggml_set_name(scan_out, "mamba_scan_out");

    return scan_out;
}

// Mamba Layer Forward Pass

struct ggml_tensor* mamba_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const MambaLayerWeights& weights,
    MambaState* state,
    const MambaLayerConfig& config) {

    if (!ctx || !input) {
        throw std::invalid_argument("Invalid arguments to mamba_layer_forward");
    }

    // 1. Layer normalization
    struct ggml_tensor* x = input;
    if (weights.norm_weight) {
        x = ggml_norm(ctx, x, 1e-5f);
        x = ggml_mul(ctx, x, weights.norm_weight);
        if (weights.norm_bias) {
            x = ggml_add(ctx, x, weights.norm_bias);
        }
        ggml_set_name(x, "mamba_norm");
    }

    // 2. Input projection: [seq_len, d_model] -> [seq_len, d_inner]
    struct ggml_tensor* x_proj = ggml_mul_mat(ctx, weights.in_proj_weight, x);
    if (weights.in_proj_bias) {
        x_proj = ggml_add(ctx, x_proj, weights.in_proj_bias);
    }
    ggml_set_name(x_proj, "mamba_in_proj");

    // Split x_proj for gating: [seq_len, d_inner] -> 2 * [seq_len, d_inner/2]
    // For simplicity, we assume x_proj contains both the input and gate in one tensor
    // In practice, this may need to be split

    // 3. SSM Convolution
    struct ggml_tensor* conv_state_ptr = (state && state->conv_state) ? state->conv_state : nullptr;
    struct ggml_tensor* x_conv = mamba_ssm_conv(
        ctx, x_proj,
        weights.conv1d_weight,
        weights.conv1d_bias,
        conv_state_ptr);

    // 4. Compute Δt (delta time)
    struct ggml_tensor* dt = ggml_mul_mat(ctx, weights.dt_weight, x_conv);
    dt = ggml_softplus(ctx, dt);  // Ensure dt > 0
    ggml_set_name(dt, "mamba_dt");

    // Clamp dt to [dt_min, dt_max]
    // Note: ggml may not have clamp, so we use softplus which ensures dt > 0

    // 5. Compute B and C projections
    // For Mamba-2, B and C are computed from the input
    // Simplified: B = x_conv * B_weight, C = x_conv * C_weight
    struct ggml_tensor* B_proj = ggml_mul_mat(ctx, weights.B, x_conv);
    ggml_set_name(B_proj, "mamba_B_proj");

    struct ggml_tensor* C_proj = ggml_mul_mat(ctx, weights.C, x_conv);
    ggml_set_name(C_proj, "mamba_C_proj");

    // 6. SSM Scan (state update)
    struct ggml_tensor* state_ptr = (state && state->state) ? state->state : nullptr;
    struct ggml_tensor* y = mamba_ssm_scan(
        ctx, x_conv, dt,
        weights.A, B_proj, C_proj,
        state_ptr,
        nullptr);  // ids for batch processing (not used in simple case)

    // 7. Skip connection (D)
    if (weights.D) {
        struct ggml_tensor* skip = ggml_mul(ctx, x_conv, weights.D);
        y = ggml_add(ctx, y, skip);
        ggml_set_name(y, "mamba_skip");
    }

    // 8. Output projection: [seq_len, d_inner] -> [seq_len, d_model]
    struct ggml_tensor* output = ggml_mul_mat(ctx, weights.out_proj_weight, y);
    if (weights.out_proj_bias) {
        output = ggml_add(ctx, output, weights.out_proj_bias);
    }
    ggml_set_name(output, "mamba_out_proj");

    // 9. Residual connection
    output = ggml_add(ctx, output, input);
    ggml_set_name(output, "mamba_residual");

    return output;
}

// Load Mamba layer weights from safetensors

MambaLayerWeights load_mamba_layer_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const std::string& prefix,
    const MambaLayerConfig& config) {

    MambaLayerWeights weights{};

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

    // Load all weights
    weights.in_proj_weight = get_tensor("in_proj.weight");
    weights.in_proj_bias = get_tensor("in_proj.bias", false);

    weights.conv1d_weight = get_tensor("conv1d.weight");
    weights.conv1d_bias = get_tensor("conv1d.bias", false);

    weights.dt_weight = get_tensor("dt_proj.weight");
    weights.A = get_tensor("A_log");  // A is stored as log(A) for numerical stability
    weights.B = get_tensor("B");
    weights.C = get_tensor("C");
    weights.D = get_tensor("D", false);

    weights.out_proj_weight = get_tensor("out_proj.weight");
    weights.out_proj_bias = get_tensor("out_proj.bias", false);

    weights.norm_weight = get_tensor("norm.weight", false);
    weights.norm_bias = get_tensor("norm.bias", false);

    return weights;
}

} // namespace safetensors
