#pragma once

#include <cstddef>
#include <cstdint>
#include <deque>
#include <functional>
#include <vector>

namespace llm_node {

class ContinuousBatchScheduler {
public:
    struct Request {
        uint64_t id{0};
        std::function<void()> prefill;
        std::function<bool()> decode_step;
    };

    void enqueue(Request request);

    bool empty() const;
    size_t prefillQueueSize() const;
    size_t decodeBatchSize() const;

    void step();
    void drain();

private:
    std::deque<Request> prefill_queue_;
    std::vector<Request> decode_batch_;
};

}  // namespace llm_node
