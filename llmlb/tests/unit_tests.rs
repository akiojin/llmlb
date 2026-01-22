//! Unit tests entrypoint for model management

#[path = "unit/gpu_model_selector_test.rs"]
mod gpu_model_selector_test;

// SPEC-66555000: エンドポイント関連テスト
#[path = "unit/endpoint_status_test.rs"]
mod endpoint_status_test;

#[path = "unit/endpoint_validation_test.rs"]
mod endpoint_validation_test;

#[path = "unit/latency_routing_test.rs"]
mod latency_routing_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
