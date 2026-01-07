// SPEC-d7feaa2c: T164-T165, T176 Replica placement and load balancing tests
#include <gtest/gtest.h>

#include "core/replica_manager.h"

namespace llm_node {
namespace {

// T164: Replica placement tests
TEST(ReplicaManagerTest, RegistersReplicaForModel) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    EXPECT_EQ(mgr.replicaCount("model_a"), 1u);

    mgr.registerReplica("model_a", 1);
    EXPECT_EQ(mgr.replicaCount("model_a"), 2u);
}

TEST(ReplicaManagerTest, TracksMultipleModels) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_b", 1);
    mgr.registerReplica("model_b", 2);

    EXPECT_EQ(mgr.replicaCount("model_a"), 1u);
    EXPECT_EQ(mgr.replicaCount("model_b"), 2u);
}

TEST(ReplicaManagerTest, UnregistersReplica) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);
    EXPECT_EQ(mgr.replicaCount("model_a"), 2u);

    mgr.unregisterReplica("model_a", 0);
    EXPECT_EQ(mgr.replicaCount("model_a"), 1u);
}

TEST(ReplicaManagerTest, ReturnsZeroForUnknownModel) {
    ReplicaManager mgr;
    EXPECT_EQ(mgr.replicaCount("nonexistent"), 0u);
}

// T165: Round-robin load balancing tests
TEST(ReplicaManagerTest, RoundRobinDistributesLoad) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);
    mgr.registerReplica("model_a", 2);

    // First round
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);
    EXPECT_EQ(mgr.selectReplica("model_a"), 1);
    EXPECT_EQ(mgr.selectReplica("model_a"), 2);

    // Wraps around
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);
}

TEST(ReplicaManagerTest, SkipsFailedReplica) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);
    mgr.registerReplica("model_a", 2);

    // Mark replica 1 as failed
    mgr.markReplicaFailed("model_a", 1);

    // Should skip GPU 1
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);
    EXPECT_EQ(mgr.selectReplica("model_a"), 2);
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);
}

TEST(ReplicaManagerTest, RecoverFailedReplica) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);

    mgr.markReplicaFailed("model_a", 1);
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);
    EXPECT_EQ(mgr.selectReplica("model_a"), 0);

    // Recover replica 1
    mgr.markReplicaHealthy("model_a", 1);

    // Now both are available - verify round-robin selects both
    int first = mgr.selectReplica("model_a");
    int second = mgr.selectReplica("model_a");

    // Both replicas should be selected in round-robin (order may vary)
    EXPECT_TRUE((first == 0 && second == 1) || (first == 1 && second == 0));
}

TEST(ReplicaManagerTest, ReturnsNegativeOneWhenNoReplicas) {
    ReplicaManager mgr;
    EXPECT_EQ(mgr.selectReplica("nonexistent"), -1);
}

TEST(ReplicaManagerTest, ReturnsNegativeOneWhenAllReplicasFailed) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);

    mgr.markReplicaFailed("model_a", 0);
    mgr.markReplicaFailed("model_a", 1);

    EXPECT_EQ(mgr.selectReplica("model_a"), -1);
}

TEST(ReplicaManagerTest, ListsReplicasForModel) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 2);
    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);

    auto replicas = mgr.getReplicas("model_a");
    ASSERT_EQ(replicas.size(), 3u);

    // Check all GPUs are present (order may vary due to set)
    std::set<int> gpu_set(replicas.begin(), replicas.end());
    EXPECT_EQ(gpu_set, std::set<int>({0, 1, 2}));
}

TEST(ReplicaManagerTest, ReturnsReplicaStatus) {
    ReplicaManager mgr;

    mgr.registerReplica("model_a", 0);
    mgr.registerReplica("model_a", 1);

    EXPECT_TRUE(mgr.isReplicaHealthy("model_a", 0));
    EXPECT_TRUE(mgr.isReplicaHealthy("model_a", 1));

    mgr.markReplicaFailed("model_a", 1);
    EXPECT_TRUE(mgr.isReplicaHealthy("model_a", 0));
    EXPECT_FALSE(mgr.isReplicaHealthy("model_a", 1));
}

}  // namespace
}  // namespace llm_node
