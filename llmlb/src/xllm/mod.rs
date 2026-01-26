//! xLLM Client Module
//!
//! SPEC-66555000: xLLM endpoint communication for model download and metadata

pub mod download;

pub use download::{
    download_model, get_download_progress, DownloadError, DownloadProgressResponse, DownloadRequest,
};
