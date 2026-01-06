#include "core/replica_manager.h"

#include <algorithm>

namespace llm_node {

bool ReplicaManager::addReplica(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto& model = models_[model_name];

    // 既存のレプリカをチェック
    for (const auto& replica : model.replicas) {
        if (replica.gpu_id == gpu_id) {
            return false;  // 既に存在
        }
    }

    model.replicas.push_back({gpu_id, ReplicaStatus::Available});
    return true;
}

bool ReplicaManager::removeReplica(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return false;
    }

    auto& replicas = it->second.replicas;
    auto replica_it = std::find_if(replicas.begin(), replicas.end(),
                                   [gpu_id](const Replica& r) { return r.gpu_id == gpu_id; });

    if (replica_it == replicas.end()) {
        return false;
    }

    replicas.erase(replica_it);

    // インデックスを調整
    if (it->second.next_index >= replicas.size()) {
        it->second.next_index = 0;
    }

    return true;
}

size_t ReplicaManager::replicaCount(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return 0;
    }
    return it->second.replicas.size();
}

std::optional<ReplicaStatus> ReplicaManager::getReplicaStatus(const std::string& model_name,
                                                               int gpu_id) const {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return std::nullopt;
    }

    for (const auto& replica : it->second.replicas) {
        if (replica.gpu_id == gpu_id) {
            return replica.status;
        }
    }
    return std::nullopt;
}

void ReplicaManager::setReplicaStatus(const std::string& model_name, int gpu_id,
                                       ReplicaStatus status) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return;
    }

    for (auto& replica : it->second.replicas) {
        if (replica.gpu_id == gpu_id) {
            replica.status = status;
            return;
        }
    }
}

std::optional<int> ReplicaManager::selectNextReplica(const std::string& model_name) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end() || it->second.replicas.empty()) {
        return std::nullopt;
    }

    auto& model = it->second;
    const size_t count = model.replicas.size();

    // 利用可能なレプリカを探す（ラウンドロビン）
    for (size_t attempts = 0; attempts < count; ++attempts) {
        const size_t index = (model.next_index + attempts) % count;
        const auto& replica = model.replicas[index];

        if (replica.status == ReplicaStatus::Available) {
            model.next_index = (index + 1) % count;
            return replica.gpu_id;
        }
    }

    return std::nullopt;  // 利用可能なレプリカなし
}

std::set<int> ReplicaManager::getAvailableGpus(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(mutex_);

    std::set<int> result;
    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return result;
    }

    for (const auto& replica : it->second.replicas) {
        if (replica.status == ReplicaStatus::Available) {
            result.insert(replica.gpu_id);
        }
    }
    return result;
}

}  // namespace llm_node
