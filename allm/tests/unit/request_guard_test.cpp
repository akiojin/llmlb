#include <gtest/gtest.h>

#include "runtime/state.h"

using namespace allm;

TEST(RequestGuardTest, AcquireAndRelease) {
    g_active_requests.store(0);

    {
        auto guard = RequestGuard::try_acquire();
        ASSERT_TRUE(guard.has_value());
        EXPECT_EQ(active_request_count(), 1u);
    }

    EXPECT_EQ(active_request_count(), 0u);
}

TEST(RequestGuardTest, RejectsSecondRequestWhileActive) {
    g_active_requests.store(0);

    auto guard = RequestGuard::try_acquire();
    ASSERT_TRUE(guard.has_value());
    EXPECT_EQ(active_request_count(), 1u);

    auto second = RequestGuard::try_acquire();
    EXPECT_FALSE(second.has_value());
    EXPECT_EQ(active_request_count(), 1u);

    guard.reset();
    EXPECT_EQ(active_request_count(), 0u);
}
