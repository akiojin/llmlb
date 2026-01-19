//! Router load balancing integration tests entrypoint
//!
//! NOTE: test_load_balancing.rs was deleted as part of NodeRegistry removal (SPEC-66555000)

#[path = "integration/test_metrics.rs"]
mod test_metrics;

#[path = "integration/loadbalancer_test.rs"]
mod loadbalancer_test;

// Tests are defined inside the module; this harness ensures they are built
// and executed when running `cargo test`.
