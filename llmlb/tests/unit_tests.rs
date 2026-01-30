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

// SPEC-66555000: エンドポイントタイプ関連テスト
#[path = "unit/endpoint_type_detection_test.rs"]
mod endpoint_type_detection_test;

#[path = "unit/endpoint_type_enum_test.rs"]
mod endpoint_type_enum_test;

#[path = "unit/download_status_test.rs"]
mod download_status_test;

// 認証関連テスト
#[path = "unit/password_test.rs"]
mod password_test;

#[path = "unit/jwt_test.rs"]
mod jwt_test;

// SPEC-f8e3a1b7: OpenAI互換エラーレスポンステスト
#[path = "unit/openai_error_format_test.rs"]
mod openai_error_format_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
