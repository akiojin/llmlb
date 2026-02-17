//! load balancer integration tests entrypoint
//!
//! NOTE: test_load_balancing.rs was deleted as part of NodeRegistry removal (SPEC-e8e9326e)
//! NOTE: loadbalancer_test.rs was deleted - metrics-based selection removed (round-robin only)

#[path = "integration/test_metrics.rs"]
mod test_metrics;

// Tests are defined inside the module; this harness ensures they are built
// and executed when running `cargo test`.
