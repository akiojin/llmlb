#pragma once

#include <algorithm>
#include <cstddef>
#include <functional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace llm_node {

inline std::vector<std::string> normalize_stop_sequences(std::vector<std::string> sequences) {
    std::vector<std::string> out;
    out.reserve(sequences.size());
    for (auto& seq : sequences) {
        if (seq.empty()) continue;
        if (std::find(out.begin(), out.end(), seq) == out.end()) {
            out.push_back(std::move(seq));
        }
    }
    return out;
}

inline std::vector<std::string> merge_stop_sequences(const std::vector<std::string>& defaults,
                                                     const std::vector<std::string>& extra) {
    std::vector<std::string> merged;
    merged.reserve(defaults.size() + extra.size());
    merged.insert(merged.end(), defaults.begin(), defaults.end());
    merged.insert(merged.end(), extra.begin(), extra.end());
    return normalize_stop_sequences(std::move(merged));
}

inline bool apply_stop_sequences_suffix(std::string& text, const std::vector<std::string>& sequences) {
    size_t match_len = 0;
    for (const auto& seq : sequences) {
        if (seq.empty()) continue;
        if (text.size() >= seq.size() &&
            text.compare(text.size() - seq.size(), seq.size(), seq) == 0) {
            match_len = std::max(match_len, seq.size());
        }
    }
    if (match_len == 0) return false;
    text.erase(text.size() - match_len);
    return true;
}

class StopSequenceStream {
public:
    explicit StopSequenceStream(std::vector<std::string> sequences)
        : sequences_(normalize_stop_sequences(std::move(sequences))) {
        for (const auto& seq : sequences_) {
            max_len_ = std::max(max_len_, seq.size());
        }
    }

    bool stopped() const { return stopped_; }

    bool push(const std::string& chunk, const std::function<void(const std::string&)>& emit) {
        if (stopped_) return true;
        if (chunk.empty()) return false;
        if (sequences_.empty()) {
            if (emit) emit(chunk);
            return false;
        }

        pending_ += chunk;

        size_t match_len = suffix_match_length(pending_);
        if (match_len > 0) {
            size_t emit_len = pending_.size() - match_len;
            if (emit_len > 0 && emit) {
                emit(pending_.substr(0, emit_len));
            }
            pending_.clear();
            stopped_ = true;
            return true;
        }

        size_t keep = longest_suffix_prefix(pending_);
        size_t emit_len = pending_.size() > keep ? pending_.size() - keep : 0;
        if (emit_len > 0 && emit) {
            emit(pending_.substr(0, emit_len));
        }
        if (emit_len > 0) {
            pending_.erase(0, emit_len);
        }
        return false;
    }

    void flush(const std::function<void(const std::string&)>& emit) {
        if (stopped_ || pending_.empty()) return;
        if (emit) emit(pending_);
        pending_.clear();
    }

private:
    size_t suffix_match_length(const std::string& text) const {
        size_t match_len = 0;
        for (const auto& seq : sequences_) {
            if (seq.empty()) continue;
            if (text.size() >= seq.size() &&
                text.compare(text.size() - seq.size(), seq.size(), seq) == 0) {
                match_len = std::max(match_len, seq.size());
            }
        }
        return match_len;
    }

    size_t longest_suffix_prefix(const std::string& text) const {
        if (sequences_.empty() || text.empty()) return 0;
        if (max_len_ <= 1) return 0;

        size_t max_check = std::min(max_len_ - 1, text.size());
        size_t keep = 0;
        for (size_t len = 1; len <= max_check; ++len) {
            const std::string_view suffix(text.data() + (text.size() - len), len);
            for (const auto& seq : sequences_) {
                if (seq.size() < len) continue;
                if (std::string_view(seq.data(), len) == suffix) {
                    keep = len;
                    break;
                }
            }
        }
        return keep;
    }

    std::vector<std::string> sequences_;
    size_t max_len_{0};
    std::string pending_;
    bool stopped_{false};
};

}  // namespace llm_node
