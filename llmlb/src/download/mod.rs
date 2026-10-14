//! Model Download Dispatcher
//!
//! Routes download requests to the appropriate engine handler based on endpoint type.
//! Follows the dispatcher pattern used in `crate::metadata::mod.rs`.

pub mod lm_studio;
pub mod ollama;

mod dispatch;

pub use dispatch::{download_model, DownloadError, DownloadRequest};
