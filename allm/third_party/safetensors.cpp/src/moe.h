#pragma once

#include <ggml.h>
#include <memory>
#include <vector>
#include <map>
#include <string>

namespace safetensors {

/**
 * MoE (Mixture of Experts) レイヤー
 *
 * Nemotron 3で使用されるMoEアーキテクチャの実装。
 * 128個のrouted expertsと2個のshared expertsを使用し、Top-6 routingで
 * 各トークンに対して6個のexpertsを選択する。
 *
 * アーキテクチャ:
 * 1. Router: MLPベースのrouting network
 * 2. Expert selection: Top-K routing (K=6)
 * 3. Expert execution: 選択されたexpertsを並列実行
 * 4. Output mixing: Routing weightsで重み付け平均
 *
 * 参考:
 * - Mixtral MoE
 * - DeepSeek-MoE (aux-loss-free load balancing)
 * - NVIDIA Technical Report: Nemotron 3 Nano
 */

struct MoELayerWeights {
    // Router network (MLP)
    struct ggml_tensor* router_weight;    // [n_experts, d_model]
    struct ggml_tensor* router_bias;      // [n_experts]

    // Routed experts (128 experts)
    std::vector<struct ggml_tensor*> expert_up_weight;    // [d_ff, d_model] x n_routed_experts
    std::vector<struct ggml_tensor*> expert_down_weight;  // [d_model, d_ff] x n_routed_experts

    // Shared experts (2 experts, always activated)
    std::vector<struct ggml_tensor*> shared_expert_up_weight;    // [d_ff, d_model] x n_shared_experts
    std::vector<struct ggml_tensor*> shared_expert_down_weight;  // [d_model, d_ff] x n_shared_experts

    // Normalization
    struct ggml_tensor* norm_weight;      // [d_model]
    struct ggml_tensor* norm_bias;        // [d_model]
};

struct MoELayerConfig {
    int d_model;              // Model dimension (e.g., 2560)
    int d_ff;                 // Feed-forward dimension (e.g., 10240)
    int n_routed_experts;     // Number of routed experts (e.g., 128)
    int n_shared_experts;     // Number of shared experts (e.g., 2)
    int top_k;                // Number of experts to activate per token (e.g., 6)
    float router_jitter;      // Router jitter for load balancing (e.g., 0.01)
};

/**
 * Expert routing結果
 *
 * Top-K expertの選択結果とrouting weightsを保持する。
 */
struct MoERoutingResult {
    std::vector<int> expert_indices;     // Selected expert indices [top_k]
    std::vector<float> expert_weights;   // Routing weights [top_k]
};

/**
 * MoEレイヤーの順伝播
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param weights レイヤーのウェイト
 * @param config レイヤー設定
 * @return 出力テンソル [seq_len, d_model]
 */
struct ggml_tensor* moe_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const MoELayerWeights& weights,
    const MoELayerConfig& config);

/**
 * Expert routing (Top-K selection)
 *
 * Router networkを使用して、各トークンに対してTop-K expertsを選択する。
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param router_weight Router weight [n_experts, d_model]
 * @param router_bias Router bias [n_experts]
 * @param top_k 選択するexpert数
 * @return Routing logits [seq_len, n_experts]
 */
struct ggml_tensor* moe_router(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    struct ggml_tensor* router_weight,
    struct ggml_tensor* router_bias,
    int top_k);

/**
 * Top-K expert selection
 *
 * Routing logitsからTop-K expertsを選択し、routing weightsを計算する。
 *
 * @param routing_logits Routing logits [seq_len, n_experts]
 * @param top_k 選択するexpert数
 * @return MoERoutingResult (expert indices and weights)
 */
MoERoutingResult select_top_k_experts(
    struct ggml_tensor* routing_logits,
    int top_k);

/**
 * Expert execution (FFN)
 *
 * 単一expertの順伝播を実行する。
 * FFN構造: up_proj -> activation -> down_proj
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param up_weight Up projection weight [d_ff, d_model]
 * @param down_weight Down projection weight [d_model, d_ff]
 * @return 出力テンソル [seq_len, d_model]
 */
struct ggml_tensor* execute_expert(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    struct ggml_tensor* up_weight,
    struct ggml_tensor* down_weight);

/**
 * MoE output mixing
 *
 * 選択されたexpertsの出力を、routing weightsで重み付け平均する。
 *
 * @param ctx ggmlコンテキスト
 * @param expert_outputs Expert outputs [top_k, seq_len, d_model]
 * @param routing_weights Routing weights [top_k]
 * @return Mixed output [seq_len, d_model]
 */
struct ggml_tensor* mix_expert_outputs(
    struct ggml_context* ctx,
    const std::vector<struct ggml_tensor*>& expert_outputs,
    const std::vector<float>& routing_weights);

/**
 * Shared experts execution
 *
 * 常にactivateされるshared expertsを実行する。
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param shared_up_weights Shared expert up weights
 * @param shared_down_weights Shared expert down weights
 * @return Shared expert outputs (averaged) [seq_len, d_model]
 */
struct ggml_tensor* execute_shared_experts(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    const std::vector<struct ggml_tensor*>& shared_up_weights,
    const std::vector<struct ggml_tensor*>& shared_down_weights);

/**
 * MoE layer weightsをsafetensorsから読み込む
 *
 * @param ctx ggmlコンテキスト
 * @param tensors safetensorsのテンソルマップ
 * @param prefix レイヤー名のプレフィックス（例: "model.layers.0.moe"）
 * @param config レイヤー設定
 * @return MoELayerWeights
 */
MoELayerWeights load_moe_layer_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const std::string& prefix,
    const MoELayerConfig& config);

} // namespace safetensors
