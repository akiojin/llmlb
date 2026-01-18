/**
 * @file partial_load_guard_test.cpp
 * @brief T186: VRAM部分ロード障害テスト
 *
 * PartialLoadGuardクラスのユニットテスト。
 * モデルロード中のOOM検出と全解放処理をテストする。
 */

#include <gtest/gtest.h>
#include "core/partial_load_guard.h"
#include <vector>
#include <string>

using namespace allm;

class PartialLoadGuardTest : public ::testing::Test {
protected:
    void SetUp() override {
        // テスト用の解放カウンター
        release_count_ = 0;
        released_resources_.clear();
    }

    static int release_count_;
    static std::vector<std::string> released_resources_;

    static void mockRelease(const std::string& resource) {
        release_count_++;
        released_resources_.push_back(resource);
    }
};

int PartialLoadGuardTest::release_count_ = 0;
std::vector<std::string> PartialLoadGuardTest::released_resources_;

// T186-1: ガード作成と正常コミット
TEST_F(PartialLoadGuardTest, CreateAndCommit) {
    {
        PartialLoadGuard guard;
        guard.addResource("tensor_1", [](){ /* cleanup */ });
        guard.addResource("tensor_2", [](){ /* cleanup */ });
        guard.commit();  // 正常完了をマーク
    }
    // コミット後はリソース解放されない
    EXPECT_EQ(release_count_, 0);
}

// T186-2: ガードスコープ終了時の自動解放（コミットなし）
TEST_F(PartialLoadGuardTest, AutoReleaseOnScopeExit) {
    {
        PartialLoadGuard guard;
        guard.addResource("tensor_1", [this](){ mockRelease("tensor_1"); });
        guard.addResource("tensor_2", [this](){ mockRelease("tensor_2"); });
        // commit()を呼ばずにスコープ終了
    }
    // コミットなしでスコープ終了 → 全リソース解放
    EXPECT_EQ(release_count_, 2);
    EXPECT_EQ(released_resources_.size(), 2);
}

// T186-3: 例外発生時の自動解放
TEST_F(PartialLoadGuardTest, AutoReleaseOnException) {
    try {
        PartialLoadGuard guard;
        guard.addResource("tensor_1", [this](){ mockRelease("tensor_1"); });
        guard.addResource("tensor_2", [this](){ mockRelease("tensor_2"); });
        throw std::runtime_error("Simulated OOM");
    } catch (const std::exception&) {
        // 例外キャッチ後
    }
    // 例外発生 → 全リソース解放
    EXPECT_EQ(release_count_, 2);
}

// T186-4: 解放順序（LIFO）
TEST_F(PartialLoadGuardTest, ReleaseOrderLIFO) {
    {
        PartialLoadGuard guard;
        guard.addResource("first", [this](){ mockRelease("first"); });
        guard.addResource("second", [this](){ mockRelease("second"); });
        guard.addResource("third", [this](){ mockRelease("third"); });
        // コミットなし
    }
    // LIFO順（最後に追加されたものから解放）
    ASSERT_EQ(released_resources_.size(), 3);
    EXPECT_EQ(released_resources_[0], "third");
    EXPECT_EQ(released_resources_[1], "second");
    EXPECT_EQ(released_resources_[2], "first");
}

// T186-5: 空のガード
TEST_F(PartialLoadGuardTest, EmptyGuard) {
    {
        PartialLoadGuard guard;
        // リソースを追加しない
    }
    // 何も解放されない
    EXPECT_EQ(release_count_, 0);
}

// T186-6: 部分的なリソース追加後のOOM
TEST_F(PartialLoadGuardTest, PartialAdditionThenOOM) {
    {
        PartialLoadGuard guard;
        guard.addResource("layer_0", [this](){ mockRelease("layer_0"); });
        guard.addResource("layer_1", [this](){ mockRelease("layer_1"); });
        // OOM発生をシミュレート（3番目のレイヤー追加前）
        guard.markFailed();  // 明示的に失敗をマーク
    }
    // markFailed()により即座に解放
    EXPECT_EQ(release_count_, 2);
}

// T186-7: リソース数の取得
TEST_F(PartialLoadGuardTest, ResourceCount) {
    PartialLoadGuard guard;
    EXPECT_EQ(guard.resourceCount(), 0);

    guard.addResource("a", [](){});
    EXPECT_EQ(guard.resourceCount(), 1);

    guard.addResource("b", [](){});
    EXPECT_EQ(guard.resourceCount(), 2);

    guard.commit();
    // コミット後もカウントは維持（解放はしない）
    EXPECT_EQ(guard.resourceCount(), 2);
}

// T186-8: 二重コミットの防止
TEST_F(PartialLoadGuardTest, DoubleCommitSafe) {
    PartialLoadGuard guard;
    guard.addResource("tensor", [this](){ mockRelease("tensor"); });
    guard.commit();
    guard.commit();  // 二重コミットは無害
    EXPECT_EQ(release_count_, 0);
}

// T186-9: コミット後のmarkFailedは無効
TEST_F(PartialLoadGuardTest, MarkFailedAfterCommit) {
    PartialLoadGuard guard;
    guard.addResource("tensor", [this](){ mockRelease("tensor"); });
    guard.commit();
    guard.markFailed();  // コミット済みなので無視される
    EXPECT_EQ(release_count_, 0);
}

// T186-10: 解放処理中の例外は伝播しない
TEST_F(PartialLoadGuardTest, ReleaseExceptionSuppressed) {
    {
        PartialLoadGuard guard;
        guard.addResource("throwing", [](){
            throw std::runtime_error("Release failed");
        });
        guard.addResource("normal", [this](){ mockRelease("normal"); });
        // コミットなしでスコープ終了
    }
    // 例外が発生しても他のリソースは解放される
    EXPECT_EQ(release_count_, 1);
    EXPECT_EQ(released_resources_[0], "normal");
}

// T186-11: isCommitted状態の確認
TEST_F(PartialLoadGuardTest, IsCommittedState) {
    PartialLoadGuard guard;
    EXPECT_FALSE(guard.isCommitted());

    guard.addResource("tensor", [](){});
    EXPECT_FALSE(guard.isCommitted());

    guard.commit();
    EXPECT_TRUE(guard.isCommitted());
}

// T186-12: VRAMサイズ追跡
TEST_F(PartialLoadGuardTest, VramSizeTracking) {
    PartialLoadGuard guard;
    EXPECT_EQ(guard.totalVramBytes(), 0);

    guard.addResource("layer_0", [](){}, 1024 * 1024);  // 1MB
    EXPECT_EQ(guard.totalVramBytes(), 1024 * 1024);

    guard.addResource("layer_1", [](){}, 2 * 1024 * 1024);  // 2MB
    EXPECT_EQ(guard.totalVramBytes(), 3 * 1024 * 1024);  // 合計3MB

    guard.commit();
}

// T186-13: クリーン状態への復帰確認
TEST_F(PartialLoadGuardTest, CleanStateAfterRelease) {
    PartialLoadGuard guard;
    guard.addResource("a", [this](){ mockRelease("a"); });
    guard.addResource("b", [this](){ mockRelease("b"); });

    guard.markFailed();

    // markFailed後は全解放済み
    EXPECT_EQ(guard.resourceCount(), 0);
    EXPECT_EQ(guard.totalVramBytes(), 0);
}

// T186-14: ムーブセマンティクス
TEST_F(PartialLoadGuardTest, MoveSemantics) {
    PartialLoadGuard guard1;
    guard1.addResource("tensor", [this](){ mockRelease("tensor"); });

    PartialLoadGuard guard2 = std::move(guard1);
    // guard1は無効化され、guard2が所有権を持つ

    guard2.commit();
    EXPECT_EQ(release_count_, 0);  // コミットしたので解放されない
}

// T186-15: リソース名の取得
TEST_F(PartialLoadGuardTest, GetResourceNames) {
    PartialLoadGuard guard;
    guard.addResource("layer_0", [](){});
    guard.addResource("layer_1", [](){});
    guard.addResource("layer_2", [](){});

    auto names = guard.resourceNames();
    ASSERT_EQ(names.size(), 3);
    EXPECT_EQ(names[0], "layer_0");
    EXPECT_EQ(names[1], "layer_1");
    EXPECT_EQ(names[2], "layer_2");

    guard.commit();
}
