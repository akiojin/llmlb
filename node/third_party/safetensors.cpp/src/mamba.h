#pragma once

#include <ggml.h>
#include <memory>
#include <vector>

namespace safetensors {

/**
 * Mamba-2 State Space Model レイヤー
 *
 * Nemotron 3で使用されるMamba-2アーキテクチャの実装。
 * 従来のTransformer attentionの代わりに、State Space Modelを使用して
 * 長コンテキスト（1M-token）の効率的な処理を実現する。
 *
 * アーキテクチャ:
 * 1. Input projection (d_model -> d_inner)
 * 2. SSM Convolution (temporal convolution)
 * 3. SSM Scan (state update)
 * 4. Output projection (d_inner -> d_model)
 *
 * 参考:
 * - ggml: GGML_OP_SSM_CONV, GGML_OP_SSM_SCAN
 * - llama.cpp: src/models/mamba.cpp
 */

struct MambaLayerWeights {
    // Input projection
    struct ggml_tensor* in_proj_weight;   // [d_inner, d_model]
    struct ggml_tensor* in_proj_bias;     // [d_inner]

    // SSM parameters
    struct ggml_tensor* conv1d_weight;    // [d_inner, conv_kernel_size]
    struct ggml_tensor* conv1d_bias;      // [d_inner]
    struct ggml_tensor* dt_weight;        // [d_inner]  - Δt (delta time)
    struct ggml_tensor* A;                // [d_state, d_inner] - State matrix
    struct ggml_tensor* B;                // [d_state, d_inner] - Input matrix
    struct ggml_tensor* C;                // [d_state, d_inner] - Output matrix
    struct ggml_tensor* D;                // [d_inner] - Skip connection weight

    // Output projection
    struct ggml_tensor* out_proj_weight;  // [d_model, d_inner]
    struct ggml_tensor* out_proj_bias;    // [d_model]

    // Normalization
    struct ggml_tensor* norm_weight;      // [d_model]
    struct ggml_tensor* norm_bias;        // [d_model]
};

struct MambaLayerConfig {
    int d_model;           // Model dimension (e.g., 2560)
    int d_inner;           // Inner dimension (e.g., 5120)
    int d_state;           // State dimension (e.g., 64)
    int conv_kernel_size;  // Convolution kernel size (e.g., 4)
    float dt_rank;         // Δt projection rank
    float dt_min;          // Minimum Δt value
    float dt_max;          // Maximum Δt value
};

/**
 * Mamba State（推論時のstate保持用）
 *
 * KVキャッシュの代わりに、定数サイズのstate vectorを保持する。
 * これにより、長コンテキストでもメモリ使用量が一定になる。
 */
struct MambaState {
    struct ggml_tensor* state;  // [d_state, d_inner]
    struct ggml_tensor* conv_state;  // [conv_kernel_size, d_inner]
    int sequence_length;

    MambaState(struct ggml_context* ctx, const MambaLayerConfig& config);
    ~MambaState() = default;

    void reset();
};

/**
 * Mamba-2レイヤーの順伝播
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param weights レイヤーのウェイト
 * @param state 推論時のstate（nullptrの場合は初期化）
 * @param config レイヤー設定
 * @return 出力テンソル [seq_len, d_model]
 */
struct ggml_tensor* mamba_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const MambaLayerWeights& weights,
    MambaState* state,
    const MambaLayerConfig& config);

/**
 * SSM Convolution
 *
 * @param ctx ggmlコンテキスト
 * @param x 入力テンソル [seq_len, d_inner]
 * @param conv_weight Convolution weight [d_inner, conv_kernel_size]
 * @param conv_bias Convolution bias [d_inner]
 * @param conv_state Convolution state（推論時）
 * @return 出力テンソル [seq_len, d_inner]
 */
struct ggml_tensor* mamba_ssm_conv(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* conv_weight,
    struct ggml_tensor* conv_bias,
    struct ggml_tensor* conv_state);

/**
 * SSM Scan（状態更新）
 *
 * @param ctx ggmlコンテキスト
 * @param x 入力テンソル [seq_len, d_inner]
 * @param dt Δt (delta time) [seq_len, d_inner]
 * @param A State matrix [d_state, d_inner]
 * @param B Input matrix [seq_len, d_state]
 * @param C Output matrix [seq_len, d_state]
 * @param state SSM state
 * @param ids Token IDs (for batch processing)
 * @return 出力テンソル [seq_len, d_inner]
 */
struct ggml_tensor* mamba_ssm_scan(
    struct ggml_context* ctx,
    struct ggml_tensor* x,
    struct ggml_tensor* dt,
    struct ggml_tensor* A,
    struct ggml_tensor* B,
    struct ggml_tensor* C,
    struct ggml_tensor* state,
    struct ggml_tensor* ids);

/**
 * Mamba layer weightsをsafetensorsから読み込む
 *
 * @param ctx ggmlコンテキスト
 * @param tensors safetensorsのテンソルマップ
 * @param prefix レイヤー名のプレフィックス（例: "model.layers.0.mamba"）
 * @param config レイヤー設定
 * @return MambaLayerWeights
 */
MambaLayerWeights load_mamba_layer_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const std::string& prefix,
    const MambaLayerConfig& config);

} // namespace safetensors
