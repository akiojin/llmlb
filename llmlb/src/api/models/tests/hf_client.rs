use super::*;

#[tokio::test]
#[serial]
async fn hf_fetch_repo_siblings_transport_errors_are_5xx() {
    // Pick an unused local port to force a connection error quickly.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener
        .local_addr()
        .expect("local addr should be available")
        .port();
    drop(listener);

    let previous = std::env::var("HF_BASE_URL").ok();
    std::env::set_var("HF_BASE_URL", format!("http://127.0.0.1:{}", port));

    let client = reqwest::Client::new();
    let err = match fetch_repo_siblings(&client, "openai/gpt-oss-7b").await {
        Ok(_) => panic!("expected transport error"),
        Err(e) => e,
    };

    assert!(matches!(err, LbError::Http(_) | LbError::Timeout(_)));
    assert_ne!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);

    match previous {
        Some(v) => std::env::set_var("HF_BASE_URL", v),
        None => std::env::remove_var("HF_BASE_URL"),
    }
}

#[tokio::test]
#[serial]
async fn hf_fetch_file_bytes_transport_errors_are_5xx() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener
        .local_addr()
        .expect("local addr should be available")
        .port();
    drop(listener);

    let previous = std::env::var("HF_BASE_URL").ok();
    std::env::set_var("HF_BASE_URL", format!("http://127.0.0.1:{}", port));

    let client = reqwest::Client::new();
    let err = match fetch_hf_file_bytes(&client, "openai/gpt-oss-7b", "model.gguf").await {
        Ok(_) => panic!("expected transport error"),
        Err(e) => e,
    };

    assert!(matches!(err, LbError::Http(_) | LbError::Timeout(_)));
    assert_ne!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);

    match previous {
        Some(v) => std::env::set_var("HF_BASE_URL", v),
        None => std::env::remove_var("HF_BASE_URL"),
    }
}

// ===== extract_filename_from_hf_url tests =====

#[test]
fn test_extract_filename_from_hf_url_resolve() {
    let result =
        extract_filename_from_hf_url("https://huggingface.co/org/repo/resolve/main/model.gguf");
    assert_eq!(result, Some("model.gguf".to_string()));
}

#[test]
fn test_extract_filename_from_hf_url_blob() {
    let result = extract_filename_from_hf_url(
        "https://huggingface.co/org/repo/blob/main/subdir/model.safetensors",
    );
    assert_eq!(result, Some("subdir/model.safetensors".to_string()));
}

#[test]
fn test_extract_filename_from_hf_url_raw() {
    let result =
        extract_filename_from_hf_url("https://huggingface.co/org/repo/raw/main/config.json");
    assert_eq!(result, Some("config.json".to_string()));
}

#[test]
fn test_extract_filename_from_hf_url_no_marker() {
    assert!(extract_filename_from_hf_url("https://huggingface.co/org/repo").is_none());
}

#[test]
fn test_extract_filename_from_hf_url_plain_repo() {
    assert!(extract_filename_from_hf_url("org/repo").is_none());
}

// ===== hf_base_url tests =====

#[test]
#[serial]
fn test_hf_base_url_default() {
    let prev = std::env::var("HF_BASE_URL").ok();
    std::env::remove_var("HF_BASE_URL");
    let url = hf_base_url();
    assert_eq!(url, "https://huggingface.co");
    if let Some(v) = prev {
        std::env::set_var("HF_BASE_URL", v);
    }
}

#[test]
#[serial]
fn test_hf_base_url_custom_strips_trailing_slash() {
    let prev = std::env::var("HF_BASE_URL").ok();
    std::env::set_var("HF_BASE_URL", "https://custom.example.com/");
    let url = hf_base_url();
    assert_eq!(url, "https://custom.example.com");
    match prev {
        Some(v) => std::env::set_var("HF_BASE_URL", v),
        None => std::env::remove_var("HF_BASE_URL"),
    }
}

// ===== hf_resolve_url tests =====

#[test]
fn test_hf_resolve_url_format() {
    let url = hf_resolve_url("https://huggingface.co", "org/repo", "model.gguf");
    assert_eq!(
        url,
        "https://huggingface.co/org/repo/resolve/main/model.gguf"
    );
}

// --- extract_filename_from_hf_url edge cases ---

#[test]
fn test_extract_filename_from_hf_url_empty_after_marker() {
    // /resolve/main/ with nothing after
    assert!(
        extract_filename_from_hf_url("https://huggingface.co/org/repo/resolve/main/").is_none()
    );
}

#[test]
fn test_extract_filename_from_hf_url_nested_path() {
    let result = extract_filename_from_hf_url(
        "https://huggingface.co/org/repo/resolve/main/path/to/model.gguf",
    );
    assert_eq!(result, Some("path/to/model.gguf".to_string()));
}

// --- hf_resolve_url edge cases ---

#[test]
fn test_hf_resolve_url_with_nested_filename() {
    let url = hf_resolve_url(
        "https://huggingface.co",
        "org/repo",
        "subdir/model.safetensors",
    );
    assert_eq!(
        url,
        "https://huggingface.co/org/repo/resolve/main/subdir/model.safetensors"
    );
}

#[test]
fn test_hf_resolve_url_empty_filename() {
    let url = hf_resolve_url("https://huggingface.co", "org/repo", "");
    assert_eq!(url, "https://huggingface.co/org/repo/resolve/main/");
}
