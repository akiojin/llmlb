#pragma once

#include <mutex>
#include <optional>
#include <set>
#include <string>
#include <unordered_map>
#include <vector>

namespace llm_node {

/// レプリカの状態
enum class ReplicaStatus {
    Available,  // 利用可能
    Busy,       // 処理中
    Failed      // 障害
};

/// 同一モデルの複数GPUレプリカを管理するクラス
/// T164: レプリカ配置の実装
/// T165: ラウンドロビン負荷分散の実装
class ReplicaManager {
public:
    ReplicaManager() = default;

    /// モデルにレプリカを追加
    /// @param model_name モデル名
    /// @param gpu_id GPU ID
    /// @return 追加成功した場合true（既存の場合false）
    bool addReplica(const std::string& model_name, int gpu_id);

    /// モデルからレプリカを削除
    /// @param model_name モデル名
    /// @param gpu_id GPU ID
    /// @return 削除成功した場合true
    bool removeReplica(const std::string& model_name, int gpu_id);

    /// モデルのレプリカ数を取得
    size_t replicaCount(const std::string& model_name) const;

    /// レプリカの状態を取得
    std::optional<ReplicaStatus> getReplicaStatus(const std::string& model_name, int gpu_id) const;

    /// レプリカの状態を設定
    void setReplicaStatus(const std::string& model_name, int gpu_id, ReplicaStatus status);

    /// ラウンドロビンで次のレプリカを選択
    /// 障害/ビジー状態のレプリカはスキップ
    /// @return 選択されたGPU ID（利用可能なレプリカがない場合はnullopt）
    std::optional<int> selectNextReplica(const std::string& model_name);

    /// 利用可能なGPU一覧を取得（Available状態のレプリカのみ）
    std::set<int> getAvailableGpus(const std::string& model_name) const;

private:
    struct Replica {
        int gpu_id;
        ReplicaStatus status{ReplicaStatus::Available};
    };

    struct ModelReplicas {
        std::vector<Replica> replicas;
        size_t next_index{0};  // ラウンドロビン用インデックス
    };

    mutable std::mutex mutex_;
    std::unordered_map<std::string, ModelReplicas> models_;
};

}  // namespace llm_node
