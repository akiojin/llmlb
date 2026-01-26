//! Test image fixtures for Vision API tests
//!
//! SPEC-f8e3a1b7: Vision test environment fixtures
//!
//! This module provides Base64-encoded test images for use in Vision API tests.
//! All images are valid PNG files that can be used with the OpenAI-compatible
//! Vision API endpoints.

/// 100x100 pixel solid red PNG image (Base64 encoded)
///
/// This is a simple test image suitable for Vision API testing.
/// File size: ~200 bytes (compressed)
pub const TEST_IMAGE_100X100_RED_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAGQAAABkCAIAAAD/gAIDAAAAkElEQVR42u3QMQ0AAAjAsPk3DRb4eJpUQZviSIEsWbJkyUKBLFmyZMlCgSxZsmTJQoEsWbJkyUKBLFmyZMlCgSxZsmTJQoEsWbJkyUKBLFmyZMlCgSxZsmTJQoEsWbJkyUKBLFmyZMlCgSxZsmTJQoEsWbJkyUKBLFmyZMlCgSxZsmTJQoEsWbJkyUKBLFnvFp4t6yugc3LNAAAAAElFTkSuQmCC";

/// 1x1 pixel transparent PNG image (Base64 encoded)
///
/// Minimal test image for basic Vision API validation.
/// File size: ~68 bytes
pub const TEST_IMAGE_1X1_TRANSPARENT_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Returns a data URI for the 100x100 test image
///
/// # Example
/// ```ignore
/// let image_url = test_image_data_uri();
/// // "data:image/png;base64,iVBORw0KGgoAAAA..."
/// ```
pub fn test_image_data_uri() -> String {
    format!("data:image/png;base64,{}", TEST_IMAGE_100X100_RED_PNG)
}

/// Returns a data URI for the 1x1 transparent test image
#[allow(dead_code)]
pub fn test_image_tiny_data_uri() -> String {
    format!("data:image/png;base64,{}", TEST_IMAGE_1X1_TRANSPARENT_PNG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    #[test]
    fn test_100x100_image_is_valid_base64() {
        let decoded = STANDARD.decode(TEST_IMAGE_100X100_RED_PNG);
        assert!(decoded.is_ok(), "100x100 image should be valid Base64");

        let bytes = decoded.unwrap();
        // PNG signature: 89 50 4E 47 0D 0A 1A 0A
        assert!(
            bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            "100x100 image should have valid PNG signature"
        );
    }

    #[test]
    fn test_1x1_image_is_valid_base64() {
        let decoded = STANDARD.decode(TEST_IMAGE_1X1_TRANSPARENT_PNG);
        assert!(decoded.is_ok(), "1x1 image should be valid Base64");

        let bytes = decoded.unwrap();
        assert!(
            bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            "1x1 image should have valid PNG signature"
        );
    }

    #[test]
    fn test_data_uri_format() {
        let uri = test_image_data_uri();
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(uri.contains(TEST_IMAGE_100X100_RED_PNG));
    }
}
