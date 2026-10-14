//! Model Metadata Retrieval Module
//!
//! SPEC-e8e9326e: Fetch model metadata (context_length, etc.) from various endpoint types

pub mod lm_studio;
pub mod ollama;
pub mod xllm;

mod dispatch;

pub use dispatch::{get_model_metadata, MetadataError, ModelMetadata};
