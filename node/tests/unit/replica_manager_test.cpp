// T164/T165/T176: レプリカ配置とラウンドロビン負荷分散テスト
#include <gtest/gtest.h>
#include <thread>

#include "core/replica_manager.h"

using namespace llm_node;

class ReplicaManagerTest : public ::testing::Test {
protected:
    void SetUp() override {
        manager_ = std::make_unique<ReplicaManager>();
    }

    std::unique_ptr<ReplicaManager> manager_;
};

// T164: 同一モデルの複数GPUロード

TEST_F(ReplicaManagerTest, AddReplicaToSingleGpu) {
    bool added = manager_->addReplica("model-a", 0);
    EXPECT_TRUE(added);
    EXPECT_EQ(manager_->replicaCount("model-a"), 1u);
}

TEST_F(ReplicaManagerTest, AddReplicasToMultipleGpus) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);
    manager_->addReplica("model-a", 2);

    EXPECT_EQ(manager_->replicaCount("model-a"), 3u);
}

TEST_F(ReplicaManagerTest, AddDuplicateReplicaOnSameGpuFails) {
    manager_->addReplica("model-a", 0);
    bool duplicate = manager_->addReplica("model-a", 0);

    EXPECT_FALSE(duplicate);
    EXPECT_EQ(manager_->replicaCount("model-a"), 1u);
}

TEST_F(ReplicaManagerTest, RemoveReplica) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);

    bool removed = manager_->removeReplica("model-a", 0);

    EXPECT_TRUE(removed);
    EXPECT_EQ(manager_->replicaCount("model-a"), 1u);
}

TEST_F(ReplicaManagerTest, RemoveNonExistentReplicaReturnsFalse) {
    manager_->addReplica("model-a", 0);

    bool removed = manager_->removeReplica("model-a", 99);
    EXPECT_FALSE(removed);
}

TEST_F(ReplicaManagerTest, ReplicaCountForUnknownModelIsZero) {
    EXPECT_EQ(manager_->replicaCount("unknown-model"), 0u);
}

// T164: レプリカステータス管理

TEST_F(ReplicaManagerTest, NewReplicaIsAvailable) {
    manager_->addReplica("model-a", 0);
    auto status = manager_->getReplicaStatus("model-a", 0);

    ASSERT_TRUE(status.has_value());
    EXPECT_EQ(*status, ReplicaStatus::Available);
}

TEST_F(ReplicaManagerTest, MarkReplicaAsBusy) {
    manager_->addReplica("model-a", 0);
    manager_->setReplicaStatus("model-a", 0, ReplicaStatus::Busy);

    auto status = manager_->getReplicaStatus("model-a", 0);
    ASSERT_TRUE(status.has_value());
    EXPECT_EQ(*status, ReplicaStatus::Busy);
}

TEST_F(ReplicaManagerTest, MarkReplicaAsFailed) {
    manager_->addReplica("model-a", 0);
    manager_->setReplicaStatus("model-a", 0, ReplicaStatus::Failed);

    auto status = manager_->getReplicaStatus("model-a", 0);
    ASSERT_TRUE(status.has_value());
    EXPECT_EQ(*status, ReplicaStatus::Failed);
}

// T165: ラウンドロビン負荷分散

TEST_F(ReplicaManagerTest, SelectNextReplicaRoundRobin) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);
    manager_->addReplica("model-a", 2);

    auto first = manager_->selectNextReplica("model-a");
    auto second = manager_->selectNextReplica("model-a");
    auto third = manager_->selectNextReplica("model-a");
    auto fourth = manager_->selectNextReplica("model-a");

    ASSERT_TRUE(first.has_value());
    ASSERT_TRUE(second.has_value());
    ASSERT_TRUE(third.has_value());
    ASSERT_TRUE(fourth.has_value());

    // ラウンドロビンで順番に選択される
    EXPECT_NE(*first, *second);
    EXPECT_NE(*second, *third);
    // 4回目は最初に戻る
    EXPECT_EQ(*fourth, *first);
}

TEST_F(ReplicaManagerTest, SelectNextReplicaReturnsNulloptForNoReplicas) {
    auto result = manager_->selectNextReplica("unknown-model");
    EXPECT_FALSE(result.has_value());
}

// T165: 障害レプリカのスキップ

TEST_F(ReplicaManagerTest, SelectNextReplicaSkipsFailedReplicas) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);
    manager_->addReplica("model-a", 2);

    // GPU 1をfailedにマーク
    manager_->setReplicaStatus("model-a", 1, ReplicaStatus::Failed);

    // 選択時にfailedをスキップして0と2だけを使う
    std::set<int> selected_gpus;
    for (int i = 0; i < 6; ++i) {
        auto gpu = manager_->selectNextReplica("model-a");
        ASSERT_TRUE(gpu.has_value());
        selected_gpus.insert(*gpu);
    }

    EXPECT_EQ(selected_gpus.count(1), 0u);  // GPU 1は選ばれない
    EXPECT_EQ(selected_gpus.count(0), 1u);
    EXPECT_EQ(selected_gpus.count(2), 1u);
}

TEST_F(ReplicaManagerTest, SelectNextReplicaReturnsNulloptWhenAllFailed) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);

    manager_->setReplicaStatus("model-a", 0, ReplicaStatus::Failed);
    manager_->setReplicaStatus("model-a", 1, ReplicaStatus::Failed);

    auto result = manager_->selectNextReplica("model-a");
    EXPECT_FALSE(result.has_value());
}

// T165: ビジーレプリカのスキップ

TEST_F(ReplicaManagerTest, SelectNextReplicaSkipsBusyReplicas) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);

    manager_->setReplicaStatus("model-a", 0, ReplicaStatus::Busy);

    auto selected = manager_->selectNextReplica("model-a");
    ASSERT_TRUE(selected.has_value());
    EXPECT_EQ(*selected, 1);  // GPU 0はビジーなのでGPU 1を選択
}

// T176: スレッドセーフティ

TEST_F(ReplicaManagerTest, ConcurrentAccessIsSafe) {
    const int kThreads = 4;
    const int kIterations = 100;

    // まず複数レプリカを追加
    for (int i = 0; i < 4; ++i) {
        manager_->addReplica("model-a", i);
    }

    std::vector<std::thread> threads;
    std::atomic<int> selections{0};

    for (int t = 0; t < kThreads; ++t) {
        threads.emplace_back([&]() {
            for (int i = 0; i < kIterations; ++i) {
                auto gpu = manager_->selectNextReplica("model-a");
                if (gpu.has_value()) {
                    ++selections;
                }
            }
        });
    }

    for (auto& th : threads) {
        th.join();
    }

    EXPECT_EQ(selections.load(), kThreads * kIterations);
}

// T176: 複数モデルの独立したレプリカ管理

TEST_F(ReplicaManagerTest, MultipleModelsHaveIndependentReplicas) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);
    manager_->addReplica("model-b", 2);
    manager_->addReplica("model-b", 3);

    EXPECT_EQ(manager_->replicaCount("model-a"), 2u);
    EXPECT_EQ(manager_->replicaCount("model-b"), 2u);

    // 各モデルは独立したラウンドロビンを持つ
    auto a1 = manager_->selectNextReplica("model-a");
    auto b1 = manager_->selectNextReplica("model-b");

    ASSERT_TRUE(a1.has_value());
    ASSERT_TRUE(b1.has_value());

    EXPECT_TRUE(*a1 == 0 || *a1 == 1);
    EXPECT_TRUE(*b1 == 2 || *b1 == 3);
}

// T164: 利用可能なGPU一覧の取得

TEST_F(ReplicaManagerTest, GetAvailableGpus) {
    manager_->addReplica("model-a", 0);
    manager_->addReplica("model-a", 1);
    manager_->addReplica("model-a", 2);

    manager_->setReplicaStatus("model-a", 1, ReplicaStatus::Failed);

    auto gpus = manager_->getAvailableGpus("model-a");

    EXPECT_EQ(gpus.size(), 2u);
    EXPECT_EQ(gpus.count(0), 1u);
    EXPECT_EQ(gpus.count(2), 1u);
    EXPECT_EQ(gpus.count(1), 0u);  // failedは含まれない
}
