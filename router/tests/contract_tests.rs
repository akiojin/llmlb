//! Router contract tests entrypoint

#[path = "support/mod.rs"]
pub mod support;

#[path = "contract/test_proxy_completions.rs"]
mod test_proxy_completions;

// SPEC-66555000: エンドポイント管理API契約テスト
#[path = "contract/endpoints_post_test.rs"]
mod endpoints_post_test;

#[path = "contract/endpoints_get_list_test.rs"]
mod endpoints_get_list_test;

#[path = "contract/endpoints_get_detail_test.rs"]
mod endpoints_get_detail_test;

#[path = "contract/endpoints_put_test.rs"]
mod endpoints_put_test;

#[path = "contract/endpoints_delete_test.rs"]
mod endpoints_delete_test;

#[path = "contract/endpoints_test_test.rs"]
mod endpoints_test_test;

#[path = "contract/endpoints_sync_test.rs"]
mod endpoints_sync_test;

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

#[path = "contract/openai_logging_test.rs"]
mod openai_logging_test;

#[path = "contract/models_source_test.rs"]
mod models_source_test;

#[path = "contract/vision_chat_test.rs"]
mod vision_chat_test;

#[path = "contract/vision_error_test.rs"]
mod vision_error_test;

#[path = "contract/vision_capabilities_test.rs"]
mod vision_capabilities_test;

// SPEC-24157000: Open Responses API契約テスト
#[path = "contract/responses_api_test.rs"]
mod responses_api_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
