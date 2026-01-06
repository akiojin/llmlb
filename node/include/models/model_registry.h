#pragma once

#include <vector>
#include <string>
#include <mutex>
#include "system/gpu_detector.h"

namespace llm_node {

/**
 * @class ModelRegistry
 * @brief モデル登録・管理クラス
 *
 * SPEC-93536000: GPUバックエンド互換性チェック機能を追加
 */
class ModelRegistry {
public:
    void setModels(std::vector<std::string> models);
    std::vector<std::string> listModels() const;
    bool hasModel(const std::string& id) const;

    /**
     * @brief 指定GPUバックエンドで実行可能なモデル一覧を取得
     * @param backend GPUバックエンド
     * @return 実行可能なモデルID一覧
     *
     * SPEC-93536000 T2.3: 現時点ではロード済みモデルは全て互換性ありと仮定
     */
    std::vector<std::string> listExecutableModels(GpuBackend backend) const;

    /**
     * @brief モデルが指定GPUバックエンドと互換性があるか確認
     * @param model_id モデルID
     * @param backend GPUバックエンド
     * @return 互換性がある場合true
     *
     * SPEC-93536000 T2.4: ロード済みモデルは互換性あり、未登録は非互換
     */
    bool isCompatible(const std::string& model_id, GpuBackend backend) const;

private:
    mutable std::mutex mutex_;
    std::vector<std::string> models_;
};

}  // namespace llm_node
