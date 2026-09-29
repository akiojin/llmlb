//! Endpoint Type Detection Module
//!
//! SPEC-e8e9326e: Automatic endpoint type detection
//!
//! Detection priority: xLLM > LM Studio > Ollama > vLLM > llama.cpp > OpenAI-compatible

mod llama_cpp;
mod lm_studio;
mod ollama;
mod vllm;
mod xllm;
pub use llama_cpp::detect_llamacpp;
pub use lm_studio::detect_lm_studio;
pub use ollama::detect_ollama;
pub use vllm::detect_vllm;
pub use xllm::detect_xllm;

mod detect;

pub use detect::{detect_endpoint_type_with_client, DetectionError, DetectionResult};
