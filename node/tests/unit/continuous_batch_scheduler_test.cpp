#include <gtest/gtest.h>

#include <string>
#include <vector>

#include "core/continuous_batch_scheduler.h"

namespace llm_node {
namespace {

ContinuousBatchScheduler::Request make_request(const std::string& name,
                                               int* remaining_steps,
                                               std::vector<std::string>* events) {
    ContinuousBatchScheduler::Request request;
    request.prefill = [name, events]() {
        events->push_back("prefill:" + name);
    };
    request.decode_step = [name, remaining_steps, events]() {
        events->push_back("decode:" + name);
        if (*remaining_steps > 0) {
            (*remaining_steps) -= 1;
        }
        return *remaining_steps > 0;
    };
    return request;
}

TEST(ContinuousBatchSchedulerTest, ProcessesPrefillBeforeDecode) {
    ContinuousBatchScheduler scheduler;
    std::vector<std::string> events;

    int a_steps = 1;
    int b_steps = 1;
    scheduler.enqueue(make_request("A", &a_steps, &events));
    scheduler.enqueue(make_request("B", &b_steps, &events));

    scheduler.step();

    std::vector<std::string> expected = {
        "prefill:A",
        "prefill:B",
        "decode:A",
        "decode:B",
    };
    EXPECT_EQ(events, expected);
    EXPECT_TRUE(scheduler.empty());
}

TEST(ContinuousBatchSchedulerTest, AddsRequestBetweenDecodeSteps) {
    ContinuousBatchScheduler scheduler;
    std::vector<std::string> events;

    int a_steps = 2;
    scheduler.enqueue(make_request("A", &a_steps, &events));

    scheduler.step();

    int b_steps = 1;
    scheduler.enqueue(make_request("B", &b_steps, &events));

    scheduler.step();

    std::vector<std::string> expected = {
        "prefill:A",
        "decode:A",
        "prefill:B",
        "decode:A",
        "decode:B",
    };
    EXPECT_EQ(events, expected);
    EXPECT_TRUE(scheduler.empty());
}

}  // namespace
}  // namespace llm_node
