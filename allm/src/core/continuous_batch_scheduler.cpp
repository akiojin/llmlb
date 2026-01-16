#include "core/continuous_batch_scheduler.h"

namespace llm_node {

void ContinuousBatchScheduler::enqueue(Request request) {
    prefill_queue_.push_back(std::move(request));
}

bool ContinuousBatchScheduler::empty() const {
    return prefill_queue_.empty() && decode_batch_.empty();
}

size_t ContinuousBatchScheduler::prefillQueueSize() const {
    return prefill_queue_.size();
}

size_t ContinuousBatchScheduler::decodeBatchSize() const {
    return decode_batch_.size();
}

void ContinuousBatchScheduler::removeCancelledRequests() {
    // Remove cancelled requests from prefill queue
    auto prefill_it = prefill_queue_.begin();
    while (prefill_it != prefill_queue_.end()) {
        if (prefill_it->is_cancelled && prefill_it->is_cancelled()) {
            prefill_it = prefill_queue_.erase(prefill_it);
        } else {
            ++prefill_it;
        }
    }

    // Remove cancelled requests from decode batch
    auto decode_it = decode_batch_.begin();
    while (decode_it != decode_batch_.end()) {
        if (decode_it->is_cancelled && decode_it->is_cancelled()) {
            decode_it = decode_batch_.erase(decode_it);
        } else {
            ++decode_it;
        }
    }
}

void ContinuousBatchScheduler::step() {
    // T138: Check cancel flag before processing
    removeCancelledRequests();

    if (!prefill_queue_.empty()) {
        while (!prefill_queue_.empty()) {
            Request request = std::move(prefill_queue_.front());
            prefill_queue_.pop_front();

            // T139: Skip cancelled requests immediately before prefill
            if (request.is_cancelled && request.is_cancelled()) {
                continue;
            }

            if (request.prefill) {
                request.prefill();
            }
            decode_batch_.push_back(std::move(request));
        }
    }

    if (decode_batch_.empty()) {
        return;
    }

    std::vector<Request> remaining;
    remaining.reserve(decode_batch_.size());
    for (auto& request : decode_batch_) {
        // T139: Skip cancelled requests immediately before decode step
        if (request.is_cancelled && request.is_cancelled()) {
            continue;
        }

        bool keep = false;
        if (request.decode_step) {
            keep = request.decode_step();
        }
        if (keep) {
            remaining.push_back(std::move(request));
        }
    }
    decode_batch_.swap(remaining);
}

void ContinuousBatchScheduler::drain() {
    while (!empty()) {
        step();
    }
}

bool ContinuousBatchScheduler::cancel(uint64_t request_id) {
    // T140: Remove request from batch without affecting others
    bool found = false;

    // Check prefill queue
    auto prefill_it = prefill_queue_.begin();
    while (prefill_it != prefill_queue_.end()) {
        if (prefill_it->id == request_id) {
            prefill_it = prefill_queue_.erase(prefill_it);
            found = true;
        } else {
            ++prefill_it;
        }
    }

    // Check decode batch
    auto decode_it = decode_batch_.begin();
    while (decode_it != decode_batch_.end()) {
        if (decode_it->id == request_id) {
            decode_it = decode_batch_.erase(decode_it);
            found = true;
        } else {
            ++decode_it;
        }
    }

    return found;
}

size_t ContinuousBatchScheduler::cancelledCount() const {
    size_t count = 0;
    for (const auto& req : prefill_queue_) {
        if (req.is_cancelled && req.is_cancelled()) {
            ++count;
        }
    }
    for (const auto& req : decode_batch_) {
        if (req.is_cancelled && req.is_cancelled()) {
            ++count;
        }
    }
    return count;
}

}  // namespace llm_node
