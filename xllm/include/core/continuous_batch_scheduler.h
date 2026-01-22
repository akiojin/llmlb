#pragma once

#include <cstddef>
#include <cstdint>
#include <deque>
#include <functional>
#include <vector>

namespace xllm {

class ContinuousBatchScheduler {
public:
    struct Request {
        uint64_t id{0};
        std::function<void()> prefill;
        std::function<bool()> decode_step;
        std::function<bool()> is_cancelled;
    };

    void enqueue(Request request);

    bool empty() const;
    size_t prefillQueueSize() const;
    size_t decodeBatchSize() const;

    void step();
    void drain();

    bool cancel(uint64_t request_id);
    size_t cancelledCount() const;

private:
    void removeCancelledRequests();

    std::deque<Request> prefill_queue_;
    std::vector<Request> decode_batch_;
};

}  // namespace xllm
