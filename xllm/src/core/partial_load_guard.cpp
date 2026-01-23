/**
 * @file partial_load_guard.cpp
 * @brief T179: 部分ロード時VRAM不足の即時全解放
 *
 * モデルロード中のリソース追跡とOOM時の全解放を管理するRAIIガード。
 */

#include "core/partial_load_guard.h"
#include <spdlog/spdlog.h>

namespace xllm {

PartialLoadGuard::~PartialLoadGuard() {
    if (!committed_ && !resources_.empty()) {
        releaseAll();
    }
}

PartialLoadGuard::PartialLoadGuard(PartialLoadGuard&& other) noexcept
    : resources_(std::move(other.resources_)),
      total_vram_bytes_(other.total_vram_bytes_),
      committed_(other.committed_) {
    // 移動元を無効化
    other.resources_.clear();
    other.total_vram_bytes_ = 0;
    other.committed_ = true;  // 移動元は解放しない
}

PartialLoadGuard& PartialLoadGuard::operator=(PartialLoadGuard&& other) noexcept {
    if (this != &other) {
        // 現在のリソースを解放（未コミットの場合）
        if (!committed_ && !resources_.empty()) {
            releaseAll();
        }

        resources_ = std::move(other.resources_);
        total_vram_bytes_ = other.total_vram_bytes_;
        committed_ = other.committed_;

        // 移動元を無効化
        other.resources_.clear();
        other.total_vram_bytes_ = 0;
        other.committed_ = true;
    }
    return *this;
}

void PartialLoadGuard::addResource(const std::string& name,
                                    ReleaseCallback release_callback,
                                    size_t vram_bytes) {
    resources_.push_back({name, std::move(release_callback), vram_bytes});
    total_vram_bytes_ += vram_bytes;
}

void PartialLoadGuard::commit() {
    committed_ = true;
}

void PartialLoadGuard::markFailed() {
    if (!committed_) {
        releaseAll();
    }
}

std::vector<std::string> PartialLoadGuard::resourceNames() const {
    std::vector<std::string> names;
    names.reserve(resources_.size());
    for (const auto& res : resources_) {
        names.push_back(res.name);
    }
    return names;
}

void PartialLoadGuard::releaseAll() {
    // LIFO順で解放（最後に追加されたものから）
    for (auto it = resources_.rbegin(); it != resources_.rend(); ++it) {
        try {
            if (it->release_callback) {
                it->release_callback();
            }
        } catch (const std::exception& e) {
            // 解放中の例外は抑制（ログのみ）
            spdlog::warn("PartialLoadGuard: Exception during release of '{}': {}",
                        it->name, e.what());
        } catch (...) {
            spdlog::warn("PartialLoadGuard: Unknown exception during release of '{}'",
                        it->name);
        }
    }

    // クリア
    resources_.clear();
    total_vram_bytes_ = 0;
}

}  // namespace xllm
