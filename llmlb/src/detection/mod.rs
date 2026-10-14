//! Endpoint Type Detection Module
//!
//! SPEC-e8e9326e: Automatic endpoint type detection
//!
//! Native engine identities take precedence over the generic OpenAI fallback.

mod engines;
mod llama_cpp;
mod lm_studio;
mod models_response;
mod ollama;
mod strata;
mod vllm;
mod xllm;
pub use llama_cpp::detect_llamacpp;
pub use lm_studio::detect_lm_studio;
pub use ollama::detect_ollama;
pub use vllm::detect_vllm;
pub use xllm::detect_xllm;

mod detect;

pub use detect::{detect_endpoint_type_with_client, DetectionError, DetectionResult};
