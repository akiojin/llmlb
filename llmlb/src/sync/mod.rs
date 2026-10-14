//! モデル同期モジュール
//!
//! エンドポイントからモデル一覧を取得し、DBと同期

pub mod capabilities;
pub mod parser;
pub use capabilities::{
    capabilities_to_strings, capability_from_str, detect_capabilities, push_unique_api,
    push_unique_capability, supported_apis_from_capabilities, Capability,
};
pub use parser::{parse_models_response, ParsedModel, ResponseFormat};

mod model_sync;

pub use model_sync::{calculate_diff, sync_models, sync_models_with_type, SyncError, SyncResult};
