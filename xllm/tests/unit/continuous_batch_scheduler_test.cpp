#include <gtest/gtest.h>

#include <string>
#include <vector>

#include "core/continuous_batch_scheduler.h"

namespace xllm {
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

// T138: Cancel flag check mechanism
TEST(ContinuousBatchSchedulerTest, CancelFlagSkipsRequest) {
    ContinuousBatchScheduler scheduler;
    std::vector<std::string> events;

    bool cancelled_a = false;
    bool cancelled_b = false;

    int a_steps = 2;
    int b_steps = 2;

    ContinuousBatchScheduler::Request req_a;
    req_a.id = 1;
    req_a.prefill = [&events]() { events.push_back("prefill:A"); };
    req_a.decode_step = [&events, &a_steps]() {
        events.push_back("decode:A");
        return --a_steps > 0;
    };
    req_a.is_cancelled = [&cancelled_a]() { return cancelled_a; };

    ContinuousBatchScheduler::Request req_b;
    req_b.id = 2;
    req_b.prefill = [&events]() { events.push_back("prefill:B"); };
    req_b.decode_step = [&events, &b_steps]() {
        events.push_back("decode:B");
        return --b_steps > 0;
    };
    req_b.is_cancelled = [&cancelled_b]() { return cancelled_b; };

    scheduler.enqueue(std::move(req_a));
    scheduler.enqueue(std::move(req_b));

    // Cancel A before first step
    cancelled_a = true;

    scheduler.step();

    // Only B should be processed
    std::vector<std::string> expected = {"prefill:B", "decode:B"};
    EXPECT_EQ(events, expected);
}

// T139: Immediate cancel response (skip before next token)
TEST(ContinuousBatchSchedulerTest, CancelDuringDecodeSkipsImmediately) {
    ContinuousBatchScheduler scheduler;
    std::vector<std::string> events;

    bool cancelled = false;
    int steps = 3;

    ContinuousBatchScheduler::Request req;
    req.id = 1;
    req.prefill = [&events]() { events.push_back("prefill"); };
    req.decode_step = [&events, &steps]() {
        events.push_back("decode");
        return --steps > 0;
    };
    req.is_cancelled = [&cancelled]() { return cancelled; };

    scheduler.enqueue(std::move(req));

    // First step: prefill + decode
    scheduler.step();
    EXPECT_EQ(events.size(), 2u);

    // Cancel before second decode
    cancelled = true;
    scheduler.step();

    // Should not have more decode events
    EXPECT_EQ(events.size(), 2u);
    EXPECT_TRUE(scheduler.empty());
}

// T140: Remove request from batch without affecting others
TEST(ContinuousBatchSchedulerTest, CancelByIdDoesNotAffectOthers) {
    ContinuousBatchScheduler scheduler;
    std::vector<std::string> events;

    int a_steps = 2;
    int b_steps = 2;

    ContinuousBatchScheduler::Request req_a;
    req_a.id = 1;
    req_a.prefill = [&events]() { events.push_back("prefill:A"); };
    req_a.decode_step = [&events, &a_steps]() {
        events.push_back("decode:A");
        return --a_steps > 0;
    };

    ContinuousBatchScheduler::Request req_b;
    req_b.id = 2;
    req_b.prefill = [&events]() { events.push_back("prefill:B"); };
    req_b.decode_step = [&events, &b_steps]() {
        events.push_back("decode:B");
        return --b_steps > 0;
    };

    scheduler.enqueue(std::move(req_a));
    scheduler.enqueue(std::move(req_b));

    // First step: both prefill and decode
    scheduler.step();
    events.clear();

    // Cancel request A by ID
    EXPECT_TRUE(scheduler.cancel(1));

    // Second step: only B should continue
    scheduler.step();

    std::vector<std::string> expected = {"decode:B"};
    EXPECT_EQ(events, expected);
}

// T145: Cancel immediate response test
TEST(ContinuousBatchSchedulerTest, CancelledCountTracksState) {
    ContinuousBatchScheduler scheduler;

    bool cancelled = false;

    ContinuousBatchScheduler::Request req;
    req.id = 1;
    req.prefill = []() {};
    req.decode_step = []() { return true; };
    req.is_cancelled = [&cancelled]() { return cancelled; };

    scheduler.enqueue(std::move(req));

    EXPECT_EQ(scheduler.cancelledCount(), 0u);

    cancelled = true;
    EXPECT_EQ(scheduler.cancelledCount(), 1u);
}

}  // namespace
}  // namespace xllm
