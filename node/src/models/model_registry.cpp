#include "models/model_registry.h"
#include <algorithm>

namespace llm_node {

void ModelRegistry::setModels(std::vector<std::string> models) {
    std::lock_guard<std::mutex> lock(mutex_);
    models_ = std::move(models);
}

std::vector<std::string> ModelRegistry::listModels() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return models_;
}

bool ModelRegistry::hasModel(const std::string& id) const {
    std::lock_guard<std::mutex> lock(mutex_);
    return std::find(models_.begin(), models_.end(), id) != models_.end();
}

std::vector<std::string> ModelRegistry::listExecutableModels(GpuBackend backend) const {
    // SPEC-93536000 T2.3: ロード済みモデルは全て現在のバックエンドと互換性があると仮定
    // 将来的にはモデルごとのバックエンド互換性情報を使用する予定
    (void)backend;  // 現時点では未使用
    std::lock_guard<std::mutex> lock(mutex_);
    return models_;
}

bool ModelRegistry::isCompatible(const std::string& model_id, GpuBackend backend) const {
    // SPEC-93536000 T2.4: ロード済みモデルは互換性あり、未登録は非互換
    (void)backend;  // 現時点では未使用
    return hasModel(model_id);
}

}  // namespace llm_node
