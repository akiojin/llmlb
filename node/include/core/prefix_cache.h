#pragma once

#include <chrono>
#include <cstdint>
#include <functional>
#include <list>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <unordered_map>
#include <vector>

namespace llm_node {

/// T161: Prefix Cache - 同一プレフィックスのKVキャッシュを共有
/// プロンプトハッシュをキーとしてKVキャッシュ状態を管理
class PrefixCache {
public:
    struct Entry {
        std::vector<uint8_t> kv_state;  // KVキャッシュのバイナリ状態
        size_t token_count{0};           // プレフィックスのトークン数
        size_t vram_bytes{0};            // VRAMサイズ見積もり
    };

    struct Stats {
        size_t hit_count{0};
        size_t miss_count{0};
        size_t entry_count{0};
        size_t current_vram_bytes{0};
        size_t vram_limit_bytes{0};
    };

    PrefixCache() = default;
    ~PrefixCache() = default;

    /// T162: VRAM上限を設定（バイト）
    void setVramLimit(size_t bytes);
    size_t getVramLimit() const;

    /// 現在のVRAM使用量を取得
    size_t getCurrentUsage() const;

    /// プレフィックスハッシュからKVキャッシュ状態を取得
    /// @param prefix_hash プレフィックスのハッシュ値
    /// @return KVキャッシュ状態（見つからない場合はnullopt）
    std::optional<Entry> get(const std::string& prefix_hash);

    /// KVキャッシュ状態を保存
    /// @param prefix_hash プレフィックスのハッシュ値
    /// @param kv_state KVキャッシュのバイナリ状態
    /// @param token_count プレフィックスのトークン数
    /// @param vram_bytes VRAMサイズ見積もり
    void put(const std::string& prefix_hash, std::vector<uint8_t> kv_state,
             size_t token_count, size_t vram_bytes);

    /// 全エントリをクリア
    void clear();

    /// 統計情報を取得
    Stats getStats() const;

    /// エントリ数を取得
    size_t entryCount() const;

private:
    struct InternalEntry {
        std::string hash;
        Entry data;
    };

    /// LRU削除を実行（VRAM上限超過時）
    void evictIfNeeded();

    std::list<InternalEntry> lru_;
    std::unordered_map<std::string, std::list<InternalEntry>::iterator> entries_;
    size_t vram_limit_{0};
    size_t current_usage_{0};
    mutable size_t hit_count_{0};
    mutable size_t miss_count_{0};
    mutable std::mutex mutex_;
};

/// プレフィックスのハッシュを計算
/// @param prefix プレフィックス文字列
/// @return ハッシュ値（SHA-256の16進数文字列）
std::string computePrefixHash(const std::string& prefix);

}  // namespace llm_node
