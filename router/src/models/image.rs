//! 画像データ処理ユーティリティ
//!
//! Visionリクエストの画像URL/Base64を検証し、デコード済みデータを扱う。

use base64::engine::general_purpose;
use base64::Engine;
use futures::StreamExt;
use llm_router_common::types::{ImageContent, ImageContentType, VisionCapability};
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use std::time::Duration;
use thiserror::Error;

const IMAGE_FETCH_TIMEOUT_SECS: u64 = 30;

/// 画像データ（デコード済みバイナリ）
#[derive(Debug, Clone)]
pub struct ImageData {
    /// 元のコンテンツ情報
    pub content: ImageContent,
    /// MIMEタイプ
    pub mime_type: ImageContentType,
    /// デコード済みバイナリ
    pub bytes: Vec<u8>,
}

impl ImageData {
    /// バイトサイズ
    pub fn size_bytes(&self) -> usize {
        self.bytes.len()
    }

    /// Base64 data URLに変換
    pub fn to_data_url(&self) -> String {
        let encoded = general_purpose::STANDARD.encode(&self.bytes);
        format!("data:{};base64,{}", self.mime_type.as_mime(), encoded)
    }
}

/// 画像検証エラー
#[derive(Debug, Error)]
pub enum ImageValidationError {
    /// 画像URLが不正
    #[error("Invalid image URL: {0}")]
    InvalidUrl(String),
    /// 画像形式が非対応
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),
    /// Base64デコード失敗
    #[error("Invalid base64 encoding")]
    InvalidBase64,
    /// 画像サイズ超過
    #[error("Image size exceeds limit ({size} bytes > {max} bytes)")]
    ImageTooLarge {
        /// 実サイズ
        size: usize,
        /// 上限
        max: usize,
    },
    /// 画像取得失敗
    #[error("Failed to fetch image: {0}")]
    FetchFailed(String),
}

/// 画像URLがdata URLか判定
pub fn is_data_url(url: &str) -> bool {
    url.starts_with("data:")
}

/// 画像URLがhttp/httpsか判定
pub fn is_http_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn parse_mime_type(mime: &str) -> Option<ImageContentType> {
    let mime = mime.trim().to_ascii_lowercase();
    match mime.as_str() {
        "image/jpeg" | "image/jpg" => Some(ImageContentType::Jpeg),
        "image/png" => Some(ImageContentType::Png),
        "image/gif" => Some(ImageContentType::Gif),
        "image/webp" => Some(ImageContentType::Webp),
        _ => None,
    }
}

fn guess_mime_from_url(url: &str) -> Option<ImageContentType> {
    let path = url.split('?').next().unwrap_or(url);
    mime_guess::from_path(path)
        .first_raw()
        .and_then(parse_mime_type)
}

fn ensure_size_limit(bytes_len: usize, max_bytes: u64) -> Result<(), ImageValidationError> {
    if bytes_len as u64 > max_bytes {
        return Err(ImageValidationError::ImageTooLarge {
            size: bytes_len,
            max: max_bytes as usize,
        });
    }
    Ok(())
}

fn validate_mime_type(
    mime_type: ImageContentType,
    config: &VisionCapability,
) -> Result<ImageContentType, ImageValidationError> {
    if config.supported_formats.contains(&mime_type) {
        Ok(mime_type)
    } else {
        Err(ImageValidationError::UnsupportedFormat(
            mime_type.as_mime().to_string(),
        ))
    }
}

fn parse_data_url(url: &str, config: &VisionCapability) -> Result<ImageData, ImageValidationError> {
    let payload = url.strip_prefix("data:").ok_or_else(|| {
        ImageValidationError::InvalidUrl("data URL must start with 'data:'".to_string())
    })?;

    let (mime_part, data_part) = payload.split_once(";base64,").ok_or_else(|| {
        ImageValidationError::InvalidUrl("data URL missing base64 marker".to_string())
    })?;

    let mime_type = parse_mime_type(mime_part)
        .ok_or_else(|| ImageValidationError::UnsupportedFormat(mime_part.to_string()))?;
    let mime_type = validate_mime_type(mime_type, config)?;

    let bytes = general_purpose::STANDARD
        .decode(data_part)
        .map_err(|_| ImageValidationError::InvalidBase64)?;
    ensure_size_limit(bytes.len(), config.max_image_size_bytes)?;

    Ok(ImageData {
        content: ImageContent::Base64 {
            data: data_part.to_string(),
            mime_type: Some(mime_type),
            size_bytes: Some(bytes.len() as u64),
        },
        mime_type,
        bytes,
    })
}

async fn fetch_image_url(
    client: &Client,
    url: &str,
    config: &VisionCapability,
) -> Result<ImageData, ImageValidationError> {
    if !is_http_url(url) {
        return Err(ImageValidationError::InvalidUrl(url.to_string()));
    }

    let response = client
        .get(url)
        .timeout(Duration::from_secs(IMAGE_FETCH_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| ImageValidationError::FetchFailed(e.to_string()))?;

    if !response.status().is_success() {
        return Err(ImageValidationError::FetchFailed(format!(
            "status {}",
            response.status()
        )));
    }

    let mime_header = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        .and_then(parse_mime_type);

    let mime_type = mime_header
        .or_else(|| guess_mime_from_url(url))
        .ok_or_else(|| ImageValidationError::UnsupportedFormat("unknown".to_string()))?;
    let mime_type = validate_mime_type(mime_type, config)?;

    if let Some(len) = response.content_length() {
        if len > config.max_image_size_bytes {
            return Err(ImageValidationError::ImageTooLarge {
                size: len as usize,
                max: config.max_image_size_bytes as usize,
            });
        }
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ImageValidationError::FetchFailed(e.to_string()))?;
        if bytes.len() + chunk.len() > config.max_image_size_bytes as usize {
            return Err(ImageValidationError::ImageTooLarge {
                size: bytes.len() + chunk.len(),
                max: config.max_image_size_bytes as usize,
            });
        }
        bytes.extend_from_slice(&chunk);
    }

    ensure_size_limit(bytes.len(), config.max_image_size_bytes)?;

    Ok(ImageData {
        content: ImageContent::Url {
            url: url.to_string(),
            mime_type: Some(mime_type),
            size_bytes: Some(bytes.len() as u64),
        },
        mime_type,
        bytes,
    })
}

/// 画像URLを検証し、デコード済みデータを返す
pub async fn validate_image_url(
    client: &Client,
    url: &str,
    config: &VisionCapability,
) -> Result<ImageData, ImageValidationError> {
    if is_data_url(url) {
        parse_data_url(url, config)
    } else {
        fetch_image_url(client, url, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_router_common::types::ImageContentType;

    #[test]
    fn test_parse_data_url_decodes_base64() {
        let config = VisionCapability::default();
        let data_url = "data:image/png;base64,aGVsbG8=";
        let image = parse_data_url(data_url, &config).expect("parse data url");
        assert_eq!(image.bytes, b"hello");
        assert_eq!(image.mime_type, ImageContentType::Png);
    }

    #[test]
    fn test_parse_data_url_rejects_invalid_base64() {
        let config = VisionCapability::default();
        let data_url = "data:image/png;base64,!!!INVALID!!!";
        let err = parse_data_url(data_url, &config).expect_err("should fail");
        assert!(matches!(err, ImageValidationError::InvalidBase64));
    }

    #[test]
    fn test_parse_data_url_rejects_unsupported_format() {
        let config = VisionCapability::default();
        let data_url = "data:image/tiff;base64,AAAA";
        let err = parse_data_url(data_url, &config).expect_err("should fail");
        assert!(matches!(err, ImageValidationError::UnsupportedFormat(_)));
    }

    #[test]
    fn test_parse_data_url_respects_size_limit() {
        let config = VisionCapability {
            supported_formats: vec![ImageContentType::Png],
            max_image_size_bytes: 1,
            max_image_count: 10,
        };
        let data_url = "data:image/png;base64,AAEC";
        let err = parse_data_url(data_url, &config).expect_err("should fail");
        assert!(matches!(err, ImageValidationError::ImageTooLarge { .. }));
    }

    #[test]
    fn test_to_data_url_roundtrip() {
        let config = VisionCapability::default();
        let data_url = "data:image/png;base64,aGVsbG8=";
        let image = parse_data_url(data_url, &config).expect("parse data url");
        let encoded = image.to_data_url();
        assert!(encoded.starts_with("data:image/png;base64,"));
        let roundtrip = parse_data_url(&encoded, &config).expect("roundtrip parse");
        assert_eq!(roundtrip.bytes, b"hello");
    }
}
