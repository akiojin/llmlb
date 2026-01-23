#pragma once

#include <ggml.h>
#include "../gqa.h"
#include "../moe.h"
#include <memory>
#include <vector>
#include <map>
#include <string>

namespace safetensors {
namespace glm {

/**
 * GLM-4.7 アーキテクチャ
 *
 * Z.ai (zai-org) が公式配布するsafetensors形式のLLMモデル
 *
 * モデルバリエーション:
 * - GLM-4.7: フルモデル (717GB, 92分割safetensors)
 * - GLM-4.7-Flash: 30B-A3B MoE (軽量版、効率重視)
 * - GLM-4.7-FP8: FP8量子化版 (メモリ効率重視)
 *
 * 特徴:
 * - Interleaved Thinking: 思考過程の出力機能
 * - Tool Use: τ²-Bench/BrowseCompで高性能
 * - MoE: 30B-A3B構成 (30Bパラメータ、3B active)
 *
 * 参考:
 * - HuggingFace: zai-org/GLM-4.7-Flash
 * - HuggingFace: zai-org/GLM-4.7-FP8
 */

/**
 * GLM MoE 設定
 */
struct GLMMoEConfig {
    int num_experts;           // 総エキスパート数
    int num_experts_per_tok;   // Top-K (トークンあたりのアクティブエキスパート数)
    int num_shared_experts;    // 共有エキスパート数 (常にアクティブ)

    GLMMoEConfig()
        : num_experts(8)
        , num_experts_per_tok(2)
        , num_shared_experts(0)
    {}
};

/**
 * GLM レイヤータイプ
 */
enum class GLMLayerType {
    DENSE,      // 通常のTransformerレイヤー (dense FFN)
    MOE         // MoE (Mixture of Experts) レイヤー
};

/**
 * GLM 設定
 */
struct GLMConfig {
    // Model dimensions
    int hidden_size;             // d_model (e.g., 4096)
    int num_hidden_layers;       // レイヤー数 (e.g., 32)
    int vocab_size;              // 語彙サイズ (e.g., 151552)
    int intermediate_size;       // FFN中間層サイズ (e.g., 13696)

    // Attention config
    int num_attention_heads;     // Q heads (e.g., 32)
    int num_key_value_heads;     // KV heads for GQA (e.g., 2)
    int head_dim;                // head dimension

    // Context
    int max_position_embeddings; // 最大コンテキスト長 (e.g., 131072)

    // RoPE
    float rope_theta;            // RoPE base (e.g., 10000.0)

    // Layer norm
    float layer_norm_eps;        // LayerNorm epsilon (e.g., 1e-5)
    bool use_rms_norm;           // RMSNorm使用フラグ

    // MoE config (for GLM-4.7-Flash)
    bool is_moe;                 // MoEモデルかどうか
    GLMMoEConfig moe_config;

    // FP8 config (for GLM-4.7-FP8)
    bool is_fp8;                 // FP8量子化モデルかどうか

    // Thinking mode
    bool supports_thinking;      // Interleaved Thinking対応
    int thinking_start_token_id; // <think> token ID
    int thinking_end_token_id;   // </think> token ID

    // Special tokens
    int bos_token_id;
    int eos_token_id;
    int pad_token_id;

    GLMConfig()
        : hidden_size(4096)
        , num_hidden_layers(32)
        , vocab_size(151552)
        , intermediate_size(13696)
        , num_attention_heads(32)
        , num_key_value_heads(2)
        , head_dim(128)
        , max_position_embeddings(131072)
        , rope_theta(10000.0f)
        , layer_norm_eps(1e-5f)
        , use_rms_norm(true)
        , is_moe(false)
        , is_fp8(false)
        , supports_thinking(true)
        , thinking_start_token_id(-1)
        , thinking_end_token_id(-1)
        , bos_token_id(1)
        , eos_token_id(2)
        , pad_token_id(0)
    {}
};

/**
 * GLM レイヤー重み
 */
struct GLMLayerWeights {
    // Pre-attention norm
    struct ggml_tensor* input_layernorm_weight;

    // Self-attention
    struct ggml_tensor* q_proj_weight;
    struct ggml_tensor* k_proj_weight;
    struct ggml_tensor* v_proj_weight;
    struct ggml_tensor* o_proj_weight;

    // Post-attention norm
    struct ggml_tensor* post_attention_layernorm_weight;

    // FFN (dense)
    struct ggml_tensor* gate_proj_weight;  // SwiGLU gate
    struct ggml_tensor* up_proj_weight;    // SwiGLU up
    struct ggml_tensor* down_proj_weight;  // FFN down projection

    // MoE (if applicable)
    struct ggml_tensor* moe_gate_weight;   // Router gate
    std::vector<struct ggml_tensor*> expert_gate_proj;
    std::vector<struct ggml_tensor*> expert_up_proj;
    std::vector<struct ggml_tensor*> expert_down_proj;
};

/**
 * GLM モデル重み
 */
struct GLMWeights {
    // Embedding
    struct ggml_tensor* token_embedding;    // [vocab_size, hidden_size]

    // Layers
    std::vector<GLMLayerWeights> layers;

    // Output
    struct ggml_tensor* final_layernorm_weight;
    struct ggml_tensor* lm_head_weight;     // [vocab_size, hidden_size]
};

/**
 * GLM config.json パース
 *
 * HuggingFace形式のconfig.jsonからGLM設定を読み込む。
 *
 * @param config_json config.jsonの内容（JSON文字列）
 * @return GLMConfig
 */
GLMConfig parse_glm_config(const std::string& config_json);

/**
 * GLM weights読み込み
 *
 * safetensorsファイルからGLMのウェイトを読み込む。
 *
 * @param ctx ggmlコンテキスト
 * @param tensors safetensorsのテンソルマップ
 * @param config モデル設定
 * @return GLMWeights
 */
GLMWeights load_glm_weights(
    struct ggml_context* ctx,
    const std::map<std::string, struct ggml_tensor*>& tensors,
    const GLMConfig& config);

/**
 * GLM forward pass
 *
 * @param ctx ggmlコンテキスト
 * @param input_ids 入力トークンID [seq_len]
 * @param weights モデルウェイト
 * @param config モデル設定
 * @param kv_cache KVキャッシュ (オートリグレッシブ生成用)
 * @return Logits [seq_len, vocab_size]
 */
struct ggml_tensor* glm_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input_ids,
    const GLMWeights& weights,
    const GLMConfig& config,
    void* kv_cache = nullptr);

/**
 * GLM single layer forward
 *
 * @param ctx ggmlコンテキスト
 * @param input 入力テンソル [seq_len, hidden_size]
 * @param layer_idx レイヤーインデックス
 * @param weights レイヤーウェイト
 * @param config モデル設定
 * @param kv_cache KVキャッシュ
 * @return 出力テンソル [seq_len, hidden_size]
 */
struct ggml_tensor* glm_layer_forward(
    struct ggml_context* ctx,
    struct ggml_tensor* input,
    int layer_idx,
    const GLMLayerWeights& weights,
    const GLMConfig& config,
    void* kv_cache = nullptr);

/**
 * GLM アーキテクチャ検出
 *
 * config.jsonからGLMアーキテクチャかどうかを判定する。
 *
 * @param config_json config.jsonの内容
 * @return GLMアーキテクチャならtrue
 */
bool is_glm_architecture(const std::string& config_json);

/**
 * GLM テンソル名からレイヤーインデックスを抽出
 *
 * @param tensor_name テンソル名
 * @return レイヤーインデックス (-1 if not a layer tensor)
 */
int extract_layer_index(const std::string& tensor_name);

/**
 * Thinking block parser
 *
 * 生成テキストからThinkingブロックを抽出する。
 */
struct ThinkingBlock {
    std::string content;       // 思考内容
    size_t start_pos;          // 開始位置
    size_t end_pos;            // 終了位置
};

/**
 * Parse thinking blocks from generated text
 *
 * @param text 生成テキスト
 * @param thinking_start 思考開始マーカー (e.g., "<think>")
 * @param thinking_end 思考終了マーカー (e.g., "</think>")
 * @return 抽出されたThinkingブロックのリスト
 */
std::vector<ThinkingBlock> parse_thinking_blocks(
    const std::string& text,
    const std::string& thinking_start = "<think>",
    const std::string& thinking_end = "</think>");

/**
 * Remove thinking blocks from text
 *
 * @param text 生成テキスト
 * @param thinking_start 思考開始マーカー
 * @param thinking_end 思考終了マーカー
 * @return 思考ブロックを除去したテキスト
 */
std::string remove_thinking_blocks(
    const std::string& text,
    const std::string& thinking_start = "<think>",
    const std::string& thinking_end = "</think>");

} // namespace glm
} // namespace safetensors
