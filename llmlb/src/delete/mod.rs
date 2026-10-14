//! Model Delete Dispatcher
//!
//! Routes delete requests to the appropriate engine handler based on endpoint type.
//! Follows the dispatcher pattern used in `crate::metadata::mod.rs`.

pub mod ollama;

mod dispatch;

pub use dispatch::{delete_model, DeleteError};
