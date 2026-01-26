#pragma once

#include <ggml.h>
#include "../mamba.h"
#include "../moe.h"
#include "../gqa.h"
#include <memory>
#include <vector>
#include <map>
#include <string>

namespace safetensors {
namespace nemotron3 {

/**
 * Nemotron 3 Nano アーキテクチャ
 *
 * NVIDIA Nemotron 3 Nano: ハイブリッド Mamba-Transformer MoE モデル
 *
 * アーキテクチャ:
 * - 52 layers total:
 *   - 23 MoE layers (Mixture of Experts)
 *   - 23 Mamba-2 layers (State Space Model)
 *   - 6 GQA layers (Grouped Query Attention)
 * - 3.2B active parameters (31.6B total parameters)
 * - 1M-token native context window
 *
 * Layer配置:
 * - MoEとMamba-2レイヤーが交互に配置
 * - GQAレイヤーは特定の位置に配置（critical attention needs）
 *
 * 参考:
 * - NVIDIA Technical Report: Nemotron 3 Nano (December 2025)
 * - HuggingFace: nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16
 */

enum class LayerType {
    MOE,      // Mixture of Experts
    MAMBA2,   // Mamba-2 State Space Model
    GQA       // Grouped Query Attention
};

struct Nemotron3LayerConfig {
    LayerType type;
    int layer_idx;

    union {
        MoELayerConfig moe;
        MambaLayerConfig mamba;
        GQALayerConfig gqa;
    };
};

struct Nemotron3Config {
    // Model dimensions
    int d_model;                 // e.g., 2560
    int n_layers;                // 52
    int vocab_size;              // e.g., 32000

    // Layer composition
    int n_moe_layers;            // 23
    int n_mamba_layers;          // 23
    int n_gqa_layers;            // 6

    // MoE config
    int n_routed_experts;        // 128
    int n_shared_experts;        // 2
    int moe_top_k;               // 6

    // Mamba config
    int mamba_d_state;           // 64
    int mamba_d_conv;            // 4

    // GQA config
    int n_heads;                 // e.g., 32
    int n_kv_heads;              // e.g., 8 (grouped)

    // Context
    int max_seq_len;             // 1048576 (1M tokens)

    // Layer indices for each type
    std::vector<int> moe_layer_indices;
    std::vector<int> mamba_layer_indices;
    std::vector<int> gqa_layer_indices;
};

/**
 * Nemotron 3 model weights
 *
 * すべてのレイヤーのウェイトを保持する。
 */
struct Nemotron3Weights {
    // Embedding
    struct ggml_tensor* token_embedding;    // [vocab_size, d_model]

    // Layer weights (indexed by layer_idx)
    std::vector<MoELayerWeights> moe_layers;
    std::vector<MambaLayerWeights> mamba_layers;
    std::vector<GQALayerWeights> gqa_layers;

    // Output
    struct ggml_tensor* output_norm_weight;
    struct ggml_tensor* output_norm_bias;
    struct ggml_tensor* lm_head_weight;     // [vocab_size, d_model]
};

/**
 * Nemotron 3 forward pass
 *
 * @param ctx ggmlコンテキスト
 * @param input_ids 入力トークンID [seq_len]
 * @param weights モデルウェイト
 * @param config モデル設定
 * @param mamba_states Mamba layer states (for autoregressive generation)
 * @return Logits [seq_len, vocab_size]
 */
struct ggml_tensor* nemotron3_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input_ids,
    const Nemotron3Weights& weights,
    const Nemotron3Config& config,
    std::vector<MambaState*>* mamba_states = nullptr);

/**
 * Nemotron 3 config.json パース
 *
 * HuggingFace形式のconfig.jsonからNemotron 3設定を読み込む。
 *
 * @param config_json config.jsonの内容（JSON文字列）
 * @return Nemotron3Config
 */
Nemotron3Config parse_nemotron3_config(const std::string& config_json);

/**
 * Nemotron 3 weights読み込み
 *
 * safetensorsファイルからNemotron 3のウェイトを読み込む。
 *
 * @param ctx ggmlコンテキスト
 * @param tensors safetensorsのテンソルマップ
 * @param config モデル設定
 * @return Nemotron3Weights
 */
Nemotron3Weights load_nemotron3_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const Nemotron3Config& config);

/**
 * Layer type detection
 *
 * Layer indexからlayer typeを判定する。
 *
 * @param layer_idx レイヤーインデックス
 * @param config モデル設定
 * @return LayerType
 */
LayerType get_layer_type(int layer_idx, const Nemotron3Config& config);

/**
 * Single layer forward pass
 *
 * 単一レイヤーの順伝播を実行する。
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, d_model]
 * @param layer_idx レイヤーインデックス
 * @param weights モデルウェイト
 * @param config モデル設定
 * @param mamba_state Mamba state (if layer is Mamba-2)
 * @return 出力テンソル [seq_len, d_model]
 */
struct ggml_tensor* nemotron3_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    int layer_idx,
    const Nemotron3Weights& weights,
    const Nemotron3Config& config,
    MambaState* mamba_state = nullptr);

} // namespace nemotron3
} // namespace safetensors
