//! メディア関連型定義
//!
//! 音声・画像関連のフォーマットや品質設定の型

use serde::{Deserialize, Serialize};

/// 音声フォーマット
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV (PCM)
    Wav,
    /// MP3
    #[default]
    Mp3,
    /// FLAC (ロスレス)
    Flac,
    /// Ogg Vorbis
    Ogg,
    /// Opus
    Opus,
}

/// 画像サイズ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ImageSize {
    /// 256x256
    #[serde(rename = "256x256")]
    Size256,
    /// 512x512
    #[serde(rename = "512x512")]
    Size512,
    /// 1024x1024 (デフォルト)
    #[default]
    #[serde(rename = "1024x1024")]
    Size1024,
    /// 1792x1024 (横長)
    #[serde(rename = "1792x1024")]
    Size1792x1024,
    /// 1024x1792 (縦長)
    #[serde(rename = "1024x1792")]
    Size1024x1792,
}

/// 画像品質
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    /// 標準品質 (デフォルト)
    #[default]
    Standard,
    /// 高品質
    Hd,
}

/// 画像スタイル
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    /// 鮮やかなスタイル (デフォルト)
    #[default]
    Vivid,
    /// 自然なスタイル
    Natural,
}

/// 画像レスポンス形式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageResponseFormat {
    /// URL形式 (デフォルト)
    #[default]
    Url,
    /// Base64エンコード形式
    #[serde(rename = "b64_json")]
    B64Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_serialization() {
        assert_eq!(serde_json::to_string(&AudioFormat::Wav).unwrap(), "\"wav\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Mp3).unwrap(), "\"mp3\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Flac).unwrap(),
            "\"flac\""
        );
        assert_eq!(serde_json::to_string(&AudioFormat::Ogg).unwrap(), "\"ogg\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Opus).unwrap(),
            "\"opus\""
        );
    }

    #[test]
    fn test_audio_format_default() {
        let default_format: AudioFormat = Default::default();
        assert_eq!(default_format, AudioFormat::Mp3);
    }

    #[test]
    fn test_audio_format_deserialization() {
        let wav: AudioFormat = serde_json::from_str("\"wav\"").unwrap();
        assert_eq!(wav, AudioFormat::Wav);

        let mp3: AudioFormat = serde_json::from_str("\"mp3\"").unwrap();
        assert_eq!(mp3, AudioFormat::Mp3);

        let flac: AudioFormat = serde_json::from_str("\"flac\"").unwrap();
        assert_eq!(flac, AudioFormat::Flac);
    }

    #[test]
    fn test_image_size_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageSize::Size256).unwrap(),
            "\"256x256\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size512).unwrap(),
            "\"512x512\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1024).unwrap(),
            "\"1024x1024\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1792x1024).unwrap(),
            "\"1792x1024\""
        );
        assert_eq!(
            serde_json::to_string(&ImageSize::Size1024x1792).unwrap(),
            "\"1024x1792\""
        );
    }

    #[test]
    fn test_image_size_default() {
        let default_size: ImageSize = Default::default();
        assert_eq!(default_size, ImageSize::Size1024);
    }

    #[test]
    fn test_image_quality_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageQuality::Standard).unwrap(),
            "\"standard\""
        );
        assert_eq!(serde_json::to_string(&ImageQuality::Hd).unwrap(), "\"hd\"");
    }

    #[test]
    fn test_image_quality_default() {
        let default_quality: ImageQuality = Default::default();
        assert_eq!(default_quality, ImageQuality::Standard);
    }

    #[test]
    fn test_image_style_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageStyle::Vivid).unwrap(),
            "\"vivid\""
        );
        assert_eq!(
            serde_json::to_string(&ImageStyle::Natural).unwrap(),
            "\"natural\""
        );
    }

    #[test]
    fn test_image_style_default() {
        let default_style: ImageStyle = Default::default();
        assert_eq!(default_style, ImageStyle::Vivid);
    }

    #[test]
    fn test_image_response_format_serialization() {
        assert_eq!(
            serde_json::to_string(&ImageResponseFormat::Url).unwrap(),
            "\"url\""
        );
        assert_eq!(
            serde_json::to_string(&ImageResponseFormat::B64Json).unwrap(),
            "\"b64_json\""
        );
    }

    #[test]
    fn test_image_response_format_default() {
        let default_format: ImageResponseFormat = Default::default();
        assert_eq!(default_format, ImageResponseFormat::Url);
    }

    // --- 追加テスト ---

    #[test]
    fn test_audio_format_serde_roundtrip() {
        for fmt in [
            AudioFormat::Wav,
            AudioFormat::Mp3,
            AudioFormat::Flac,
            AudioFormat::Ogg,
            AudioFormat::Opus,
        ] {
            let json = serde_json::to_string(&fmt).unwrap();
            let deserialized: AudioFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, fmt);
        }
    }

    #[test]
    fn test_image_size_serde_roundtrip() {
        for size in [
            ImageSize::Size256,
            ImageSize::Size512,
            ImageSize::Size1024,
            ImageSize::Size1792x1024,
            ImageSize::Size1024x1792,
        ] {
            let json = serde_json::to_string(&size).unwrap();
            let deserialized: ImageSize = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, size);
        }
    }

    #[test]
    fn test_image_quality_serde_roundtrip() {
        for quality in [ImageQuality::Standard, ImageQuality::Hd] {
            let json = serde_json::to_string(&quality).unwrap();
            let deserialized: ImageQuality = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, quality);
        }
    }

    #[test]
    fn test_image_style_serde_roundtrip() {
        for style in [ImageStyle::Vivid, ImageStyle::Natural] {
            let json = serde_json::to_string(&style).unwrap();
            let deserialized: ImageStyle = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, style);
        }
    }

    #[test]
    fn test_image_response_format_serde_roundtrip() {
        for fmt in [ImageResponseFormat::Url, ImageResponseFormat::B64Json] {
            let json = serde_json::to_string(&fmt).unwrap();
            let deserialized: ImageResponseFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, fmt);
        }
    }
}
