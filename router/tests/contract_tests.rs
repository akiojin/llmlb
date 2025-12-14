//! Router contract tests entrypoint

#[path = "support/mod.rs"]
pub mod support;

#[path = "contract/test_node_register_gpu.rs"]
mod test_node_register_gpu;

#[path = "contract/test_proxy_completions.rs"]
mod test_proxy_completions;

#[path = "contract/models_api_test.rs"]
mod models_api_test;

#[path = "contract/chat_modal_embed.rs"]
mod chat_modal_embed;

#[path = "contract/chat_page_spec.rs"]
mod chat_page_spec;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
