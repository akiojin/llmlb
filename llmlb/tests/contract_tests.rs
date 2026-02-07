//! load balancer contract tests entrypoint

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

// SPEC-66555000: エンドポイントタイプ関連の契約テスト
#[path = "contract/endpoints_type_filter_test.rs"]
mod endpoints_type_filter_test;

#[path = "contract/endpoints_download_test.rs"]
mod endpoints_download_test;

#[path = "contract/endpoints_download_progress_test.rs"]
mod endpoints_download_progress_test;

#[path = "contract/endpoints_model_info_test.rs"]
mod endpoints_model_info_test;

#[path = "contract/models_api_test.rs"]
mod models_api_test;

#[path = "contract/chat_modal_embed.rs"]
mod chat_modal_embed;

// NOTE: chat_page_spec.rs は削除されました
// Playground機能はダッシュボード内のエンドポイント別Playgroundに移行 (#playground/:endpointId)

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

#[path = "contract/openai_request_sanitization_spec.rs"]
mod openai_request_sanitization_spec;

#[path = "contract/queueing_test.rs"]
mod queueing_test;

#[path = "contract/models_source_test.rs"]
mod models_source_test;

// SPEC-fbc50d97: リクエスト/レスポンス履歴API契約テスト
#[path = "contract/request_history_api_test.rs"]
mod request_history_api_test;

// SPEC-24157000: Open Responses API契約テスト
#[path = "contract/responses_api_test.rs"]
mod responses_api_test;

// Tests are defined inside the modules; this harness ensures they are built
// and executed when running `cargo test`.
