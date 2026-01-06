/**
 * @file partial_load_guard.h
 * @brief T179: 部分ロード時VRAM不足の即時全解放
 *
 * モデルロード中のリソース追跡とOOM時の全解放を管理するRAIIガード。
 * ロード中に例外やOOMが発生した場合、部分的にロードされたテンソルを
 * 全て解放してクリーン状態に復帰する。
 */

#pragma once

#include <functional>
#include <string>
#include <vector>
#include <cstddef>

namespace llm_node {

/**
 * @class PartialLoadGuard
 * @brief モデルロード中のリソース追跡とロールバックを管理するRAIIガード
 *
 * 使用例:
 * @code
 * PartialLoadGuard guard;
 * for (int i = 0; i < num_layers; i++) {
 *     void* tensor = load_tensor(i);
 *     guard.addResource("layer_" + std::to_string(i),
 *                       [tensor](){ free_tensor(tensor); },
 *                       tensor_size);
 * }
 * guard.commit();  // 正常完了
 * @endcode
 *
 * commit()を呼ばずにスコープを抜けると、追加された全リソースが
 * LIFO順で解放される（例外発生時も含む）。
 */
class PartialLoadGuard {
public:
    using ReleaseCallback = std::function<void()>;

    PartialLoadGuard() = default;
    ~PartialLoadGuard();

    // コピー禁止
    PartialLoadGuard(const PartialLoadGuard&) = delete;
    PartialLoadGuard& operator=(const PartialLoadGuard&) = delete;

    // ムーブ許可
    PartialLoadGuard(PartialLoadGuard&& other) noexcept;
    PartialLoadGuard& operator=(PartialLoadGuard&& other) noexcept;

    /**
     * @brief リソースを追加
     * @param name リソース名（デバッグ/ログ用）
     * @param release_callback 解放時に呼び出すコールバック
     * @param vram_bytes このリソースが使用するVRAMバイト数（オプション）
     */
    void addResource(const std::string& name,
                     ReleaseCallback release_callback,
                     size_t vram_bytes = 0);

    /**
     * @brief ロード成功をマーク（リソース解放をスキップ）
     *
     * この関数を呼ぶと、デストラクタでリソースは解放されない。
     */
    void commit();

    /**
     * @brief ロード失敗をマーク（即座に全リソースを解放）
     *
     * OOM検出時など、即座にクリーンアップが必要な場合に使用。
     * すでにcommit()済みの場合は何もしない。
     */
    void markFailed();

    /**
     * @brief コミット済みかどうか
     */
    bool isCommitted() const { return committed_; }

    /**
     * @brief 追跡中のリソース数
     */
    size_t resourceCount() const { return resources_.size(); }

    /**
     * @brief 追跡中の総VRAMバイト数
     */
    size_t totalVramBytes() const { return total_vram_bytes_; }

    /**
     * @brief リソース名の一覧を取得
     */
    std::vector<std::string> resourceNames() const;

private:
    struct Resource {
        std::string name;
        ReleaseCallback release_callback;
        size_t vram_bytes;
    };

    std::vector<Resource> resources_;
    size_t total_vram_bytes_{0};
    bool committed_{false};

    /**
     * @brief 全リソースをLIFO順で解放
     */
    void releaseAll();
};

}  // namespace llm_node
