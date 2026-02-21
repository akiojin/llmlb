//! E2E: IPアドレス記録のエンドツーエンドテスト
//!
//! SPEC-62ac4b68: リクエスト送信時にclient_ipとapi_key_idが記録されることを検証

#[path = "support/mod.rs"]
mod support;

#[path = "e2e/ip_recording_test.rs"]
mod ip_recording_test;
