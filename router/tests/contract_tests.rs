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

#[path = "contract/audio_transcriptions_test.rs"]
mod audio_transcriptions_test;

#[path = "contract/audio_speech_test.rs"]
mod audio_speech_test;

#[path = "contract/images_generations_test.rs"]
mod images_generations_test;

#[path = "contract/images_edits_test.rs"]
mod images_edits_test;

#[path = "contract/images_variations_test.rs"]
mod images_variations_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
