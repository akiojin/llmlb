//! Integration tests entrypoint for model management
//! （現行仕様では手動配布/旧自動配布テストを削除済み）

#[path = "integration/model_info_test.rs"]
mod model_info_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
