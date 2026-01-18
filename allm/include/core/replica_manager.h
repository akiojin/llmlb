#pragma once

#include <mutex>
#include <set>
#include <string>
#include <unordered_map>
#include <vector>

namespace allm {

/// T164-T165: Replica placement and round-robin load balancing
class ReplicaManager {
public:
    /// Register a replica (GPU ID) for a model
    void registerReplica(const std::string& model_name, int gpu_id);

    /// Unregister a replica
    void unregisterReplica(const std::string& model_name, int gpu_id);

    /// Get the number of replicas for a model
    size_t replicaCount(const std::string& model_name) const;

    /// Get all replica GPU IDs for a model
    std::vector<int> getReplicas(const std::string& model_name) const;

    /// Select next replica using round-robin, skipping failed replicas
    /// Returns -1 if no healthy replicas available
    int selectReplica(const std::string& model_name);

    /// Mark a replica as failed (will be skipped in selection)
    void markReplicaFailed(const std::string& model_name, int gpu_id);

    /// Mark a replica as healthy (will be included in selection)
    void markReplicaHealthy(const std::string& model_name, int gpu_id);

    /// Check if a replica is healthy
    bool isReplicaHealthy(const std::string& model_name, int gpu_id) const;

private:
    struct ModelReplicas {
        std::set<int> gpu_ids;         // All registered GPU IDs
        std::set<int> failed_gpus;     // Failed GPU IDs
        size_t next_index{0};          // Round-robin index
    };

    mutable std::mutex mutex_;
    std::unordered_map<std::string, ModelReplicas> models_;

    /// Get healthy replicas sorted by GPU ID
    std::vector<int> getHealthyReplicas(const ModelReplicas& replicas) const;
};

}  // namespace allm
