//! System Info Retrieval Module
//!
//! SPEC-e8e9326e: Fetch GPU/device information from various endpoint types
//!
//! Different endpoint types expose system information via different APIs:
//! - xLLM: GET /api/system
//! - Ollama: GET /api/system
//! - llama.cpp: GET /slots (preferred) or GET /metrics (fallback)
//! - vLLM: Not supported
//! - OpenAI-compatible: Not supported

pub mod llamacpp;
pub mod xllm;

mod fetch;

pub use fetch::get_endpoint_system_info;
