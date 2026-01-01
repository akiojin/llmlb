#pragma once

#include <string>
#include <vector>
#include <functional>
#include <memory>
#include <stdexcept>
#include <filesystem>

#include "core/engine_types.h"
#include "core/engine_host.h"
#include "core/engine_registry.h"

namespace llm_node {

// 前方宣言
class LlamaManager;
class ModelStorage;
class ModelSync;
class ModelResolver;
class VisionProcessor;
struct ModelDescriptor;

class InferenceEngine {
public:
    /// コンストラクタ: LlamaManager, ModelStorage, ModelSync/ModelResolver への参照を注入
    InferenceEngine(LlamaManager& manager, ModelStorage& model_storage, ModelSync* model_sync = nullptr,
                    ModelResolver* model_resolver = nullptr);

    /// デフォルトコンストラクタ（互換性維持、スタブモード）
    /// VisionProcessor完全型のために.cppで定義
    InferenceEngine();

    /// デストラクタ（VisionProcessor完全型のために.cppで定義）
    ~InferenceEngine();

    /// チャット生成（llama.cpp API使用）
    std::string generateChat(const std::vector<ChatMessage>& messages,
                            const std::string& model,
                            const InferenceParams& params = {}) const;

    /// 画像付きチャット生成（mtmd使用）
    std::string generateChatWithImages(const std::vector<ChatMessage>& messages,
                                       const std::vector<std::string>& image_urls,
                                       const std::string& model,
                                       const InferenceParams& params = {}) const;

    /// テキスト補完
    std::string generateCompletion(const std::string& prompt,
                                   const std::string& model,
                                   const InferenceParams& params = {}) const;

    /// ストリーミングチャット生成
    /// on_token コールバックは各トークン生成時に呼ばれる
    /// 完了時は "[DONE]" を送信
    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        const std::string& model,
        const InferenceParams& params,
        const std::function<void(const std::string&)>& on_token) const;

    /// 旧API互換（max_tokens のみ指定）
    std::vector<std::string> generateChatStream(
        const std::vector<ChatMessage>& messages,
        size_t max_tokens,
        const std::function<void(const std::string&)>& on_token) const;

    /// バッチ推論（複数プロンプトを処理）
    std::vector<std::vector<std::string>> generateBatch(
        const std::vector<std::string>& prompts,
        size_t max_tokens) const;

    /// 簡易トークン生成（スペース区切り、互換性維持）
    std::vector<std::string> generateTokens(const std::string& prompt,
                                            size_t max_tokens = 5) const;

    /// サンプリング（互換性維持）
    std::string sampleNextToken(const std::vector<std::string>& tokens) const;

    /// Embedding生成
    /// @param input テキスト入力（単一または複数）
    /// @param model モデル名
    /// @return 各入力に対するembeddingベクトル
    std::vector<std::vector<float>> generateEmbeddings(
        const std::vector<std::string>& inputs,
        const std::string& model) const;

    /// 依存関係が注入されているか確認
    bool isInitialized() const { return manager_ != nullptr && model_storage_ != nullptr; }

    /// モデルをロード（ローカルまたは外部/プロキシ解決）
    /// @return ロード結果（成功/失敗）
    ModelLoadResult loadModel(const std::string& model_name);

    /// モデルの最大コンテキストサイズを取得
    size_t getModelMaxContext() const { return model_max_ctx_; }

    /// モデルが利用可能かを判定（エンジン/メタデータに基づく）
    bool isModelSupported(const ModelDescriptor& descriptor) const;

    /// エンジンプラグインをロードする
    bool loadEnginePlugins(const std::filesystem::path& directory, std::string& error);

private:
    LlamaManager* manager_{nullptr};
    ModelStorage* model_storage_{nullptr};
    ModelSync* model_sync_{nullptr};
    ModelResolver* model_resolver_{nullptr};
    EngineHost engine_host_;
    std::unique_ptr<EngineRegistry> engines_;
    size_t model_max_ctx_{4096};  // モデルの最大コンテキストサイズ
    mutable std::unique_ptr<VisionProcessor> vision_processor_{nullptr};

    /// チャットメッセージからプロンプト文字列を構築
    std::string buildChatPrompt(const std::vector<ChatMessage>& messages) const;

    /// モデルパス解決（ModelResolver優先）
    std::string resolveModelPath(const std::string& model_name, std::string* error_message = nullptr) const;
};

}  // namespace llm_node
