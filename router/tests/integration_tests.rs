//! Integration tests entrypoint for model management
//! （現行仕様では手動配布/旧自動配布テストを削除済み）

#[path = "support/mod.rs"]
mod support;

#[path = "integration/model_info_test.rs"]
mod model_info_test;

#[path = "integration/audio_api_test.rs"]
mod audio_api_test;

#[path = "integration/images_api_test.rs"]
mod images_api_test;

#[path = "contract/test_proxy_completions.rs"]
mod test_proxy_completions;

#[path = "integration/vision_api_test.rs"]
mod vision_api_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
