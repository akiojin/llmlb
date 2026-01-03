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

void ContinuousBatchScheduler::step() {
    if (!prefill_queue_.empty()) {
        while (!prefill_queue_.empty()) {
            Request request = std::move(prefill_queue_.front());
            prefill_queue_.pop_front();
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

}  // namespace llm_node
