//! E2E: 認証関連のエンドツーエンドテスト
//!
//! T091-T093: 認証フロー、APIキーフロー、ノードフローの完全なE2Eテスト

#[path = "support/mod.rs"]
pub mod support;

#[path = "e2e/auth_flow_test.rs"]
mod auth_flow_test;

#[path = "e2e/api_key_flow_test.rs"]
mod api_key_flow_test;

#[path = "e2e/node_flow_test.rs"]
mod node_flow_test;

#[path = "e2e/dashboard_flow_test.rs"]
mod dashboard_flow_test;

#[path = "e2e/invitation_flow_test.rs"]
mod invitation_flow_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
