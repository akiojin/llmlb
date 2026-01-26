//! Integration tests entrypoint for model management
//! （現行仕様では手動配布/旧自動配布テストを削除済み）

#[path = "support/mod.rs"]
mod support;

#[path = "integration/model_info_test.rs"]
mod model_info_test;

// NOTE: audio_api_test.rs was deleted as part of NodeRegistry removal (SPEC-66555000)
// All tests depended on deprecated /v0/internal/test/register-node endpoint

#[path = "integration/images_api_test.rs"]
mod images_api_test;

#[path = "contract/test_proxy_completions.rs"]
mod test_proxy_completions;

#[path = "integration/vision_api_test.rs"]
mod vision_api_test;

#[path = "integration/test_dashboard.rs"]
mod test_dashboard;

// SPEC-66555000: エンドポイント管理統合テスト
#[path = "integration/endpoint_registration_test.rs"]
mod endpoint_registration_test;

#[path = "integration/endpoint_health_check_test.rs"]
mod endpoint_health_check_test;

#[path = "integration/endpoint_model_sync_test.rs"]
mod endpoint_model_sync_test;

#[path = "integration/endpoint_connection_test_test.rs"]
mod endpoint_connection_test_test;

#[path = "integration/endpoint_management_test.rs"]
mod endpoint_management_test;

#[path = "integration/endpoint_name_uniqueness_test.rs"]
mod endpoint_name_uniqueness_test;

#[path = "integration/endpoint_latency_routing_test.rs"]
mod endpoint_latency_routing_test;

#[path = "integration/endpoint_auto_recovery_test.rs"]
mod endpoint_auto_recovery_test;

#[path = "integration/endpoint_viewer_access_test.rs"]
mod endpoint_viewer_access_test;

// SPEC-24157000: Open Responses API統合テスト
#[path = "integration/responses_api_test.rs"]
mod responses_api_test;

#[path = "integration/responses_streaming_test.rs"]
mod responses_streaming_test;

#[path = "integration/models_api_test.rs"]
mod models_api_test;

// SPEC-f8e3a1b7: /v0/system API統合テスト
#[path = "integration/v0_system_test.rs"]
mod v0_system_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
