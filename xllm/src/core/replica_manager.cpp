// SPEC-d7feaa2c: T164-T165 Replica placement and round-robin load balancing
#include "core/replica_manager.h"

#include <spdlog/spdlog.h>

namespace xllm {

void ReplicaManager::registerReplica(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto& model = models_[model_name];
    model.gpu_ids.insert(gpu_id);

    spdlog::debug("Registered replica for {} on GPU {}", model_name, gpu_id);
}

void ReplicaManager::unregisterReplica(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return;
    }

    it->second.gpu_ids.erase(gpu_id);
    it->second.failed_gpus.erase(gpu_id);

    spdlog::debug("Unregistered replica for {} on GPU {}", model_name, gpu_id);
}

size_t ReplicaManager::replicaCount(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return 0;
    }
    return it->second.gpu_ids.size();
}

std::vector<int> ReplicaManager::getReplicas(const std::string& model_name) const {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return {};
    }

    return std::vector<int>(it->second.gpu_ids.begin(), it->second.gpu_ids.end());
}

std::vector<int> ReplicaManager::getHealthyReplicas(const ModelReplicas& replicas) const {
    std::vector<int> healthy;
    for (int gpu : replicas.gpu_ids) {
        if (replicas.failed_gpus.find(gpu) == replicas.failed_gpus.end()) {
            healthy.push_back(gpu);
        }
    }
    return healthy;
}

int ReplicaManager::selectReplica(const std::string& model_name) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return -1;
    }

    auto healthy = getHealthyReplicas(it->second);
    if (healthy.empty()) {
        spdlog::warn("No healthy replicas for {}", model_name);
        return -1;
    }

    // Round-robin selection
    size_t index = it->second.next_index % healthy.size();
    it->second.next_index = index + 1;

    int selected = healthy[index];
    spdlog::debug("Selected replica GPU {} for {} (round-robin)", selected, model_name);

    return selected;
}

void ReplicaManager::markReplicaFailed(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return;
    }

    it->second.failed_gpus.insert(gpu_id);
    spdlog::warn("Replica for {} on GPU {} marked as failed", model_name, gpu_id);
}

void ReplicaManager::markReplicaHealthy(const std::string& model_name, int gpu_id) {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return;
    }

    it->second.failed_gpus.erase(gpu_id);
    spdlog::info("Replica for {} on GPU {} marked as healthy", model_name, gpu_id);
}

bool ReplicaManager::isReplicaHealthy(const std::string& model_name, int gpu_id) const {
    std::lock_guard<std::mutex> lock(mutex_);

    auto it = models_.find(model_name);
    if (it == models_.end()) {
        return false;
    }

    return it->second.failed_gpus.find(gpu_id) == it->second.failed_gpus.end();
}

}  // namespace xllm
