//! Stream provider metadata through serde while ignoring payload values.
//!
//! In particular, `IgnoredAny` uses serde_json's string-skipping path rather
//! than allocating image/base64 or generated text in a second JSON value tree.

use crate::common::usage::{TokenMeasurement, TokenSource, UsageTokens};
use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::io::{self, BufRead, Read};

#[derive(Default)]
pub(super) struct Metadata {
    pub tokens: UsageTokens,
    pub failed: bool,
}

impl Metadata {
    fn merge(&mut self, next: Self) {
        self.tokens = self.tokens.merge(&next.tokens);
        self.failed |= next.failed;
    }
}

#[derive(Deserialize)]
#[serde(field_identifier)]
enum RootKey {
    #[serde(rename = "usage", alias = "usageMetadata")]
    Usage,
    #[serde(rename = "response", alias = "message")]
    Nested,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "type")]
    Type,
    #[serde(other)]
    Other,
}

struct RootSeed<'a>(&'a mut Metadata);

impl<'de> DeserializeSeed<'de> for RootSeed<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(RootVisitor(self.0))
    }
}

struct RootVisitor<'a>(&'a mut Metadata);

impl<'de> Visitor<'de> for RootVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a provider response object or sequence")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
        while values.next_element_seed(RootSeed(&mut *self.0))?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while let Some(key) = map.next_key::<RootKey>()? {
            match key {
                RootKey::Usage => {
                    self.0.tokens = self.0.tokens.merge(&map.next_value::<UsageObject>()?.0);
                }
                RootKey::Nested => map.next_value_seed(RootSeed(&mut *self.0))?,
                RootKey::Error => {
                    self.0.failed |= map.next_value::<Option<IgnoredAny>>()?.is_some();
                }
                RootKey::Type => {
                    self.0.failed |= map.next_value::<MetadataScalar>()?.is_error;
                }
                RootKey::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(field_identifier)]
enum UsageKey {
    #[serde(
        rename = "prompt_tokens",
        alias = "input_tokens",
        alias = "promptTokenCount"
    )]
    Input,
    #[serde(
        rename = "completion_tokens",
        alias = "output_tokens",
        alias = "candidatesTokenCount"
    )]
    Output,
    #[serde(rename = "total_tokens", alias = "totalTokenCount")]
    Total,
    #[serde(other)]
    Other,
}

struct UsageObject(UsageTokens);
struct UsageVisitor;

impl<'de> Visitor<'de> for UsageVisitor {
    type Value = UsageObject;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("provider usage fields")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_str<E: de::Error>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_bool<E: de::Error>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_i64<E: de::Error>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_u64<E: de::Error>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(UsageObject(UsageTokens::default()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
        while values.next_element::<IgnoredAny>()?.is_some() {}
        Ok(UsageObject(UsageTokens::default()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut tokens = UsageTokens::default();
        while let Some(key) = map.next_key::<UsageKey>()? {
            match key {
                UsageKey::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
                key => {
                    let count = map.next_value::<MetadataScalar>()?.count;
                    let observation = TokenMeasurement::new(count, TokenSource::Provider);
                    match key {
                        UsageKey::Input => tokens.input = tokens.input.merge(observation),
                        UsageKey::Output => tokens.output = tokens.output.merge(observation),
                        UsageKey::Total => tokens.total = tokens.total.merge(observation),
                        UsageKey::Other => unreachable!(),
                    }
                }
            }
        }
        Ok(UsageObject(tokens))
    }
}

impl<'de> Deserialize<'de> for UsageObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UsageVisitor)
    }
}

#[derive(Default)]
struct MetadataScalar {
    count: Option<u32>,
    is_error: bool,
}
struct CountVisitor;

impl<'de> Visitor<'de> for CountVisitor {
    type Value = MetadataScalar;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a nonnegative bounded integer or missing value")
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(MetadataScalar {
            count: u32::try_from(value).ok(),
            is_error: false,
        })
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(MetadataScalar {
            count: u32::try_from(value).ok(),
            is_error: false,
        })
    }
    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(MetadataScalar::default())
    }
    fn visit_bool<E: de::Error>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(MetadataScalar::default())
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(MetadataScalar {
            count: None,
            is_error: value == "error",
        })
    }
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(MetadataScalar::default())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut values: A) -> Result<Self::Value, A::Error> {
        while values.next_element::<IgnoredAny>()?.is_some() {}
        Ok(MetadataScalar::default())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(MetadataScalar::default())
    }
}

impl<'de> Deserialize<'de> for MetadataScalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(CountVisitor)
    }
}

/// Checkpoint each completed usage object while serde reads the remaining JSON.
/// A later payload EOF cannot erase observations already reported by a provider.
/// Drain after parse errors to preserve response-body backpressure.
pub(super) fn read_json(mut reader: impl Read) -> Metadata {
    let mut metadata = Metadata::default();
    {
        let mut deserializer = serde_json::Deserializer::from_reader(&mut reader);
        let _ = RootSeed(&mut metadata)
            .deserialize(&mut deserializer)
            .and_then(|()| deserializer.end());
    }
    let _ = io::copy(&mut reader, &mut io::sink());
    metadata
}

struct DataLine<'a, R> {
    reader: &'a mut R,
    ended: bool,
}

impl<R: BufRead> Read for DataLine<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.ended || out.is_empty() {
            return Ok(0);
        }
        let available = self.reader.fill_buf()?;
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(available.len());
        let len = end.min(out.len());
        if len == 0 && !available.is_empty() {
            self.reader.consume(1);
            self.ended = true;
            return Ok(0);
        }
        out[..len].copy_from_slice(&available[..len]);
        self.reader.consume(len);
        Ok(len)
    }
}

/// Read SSE data lines without retaining large delta/string values in memory.
pub(super) fn read_sse(mut reader: impl BufRead) -> Metadata {
    let mut metadata = Metadata::default();
    'lines: loop {
        let mut prefix = [0u8; 5];
        for byte in &mut prefix {
            match reader.read_exact(std::slice::from_mut(byte)) {
                Ok(()) if *byte == b'\n' => continue 'lines,
                Ok(()) => {}
                Err(_) => break 'lines,
            }
        }
        let mut line = DataLine {
            reader: &mut reader,
            ended: false,
        };
        if &prefix == b"data:" {
            metadata.merge(read_json(&mut line));
        } else {
            let _ = io::copy(&mut line, &mut io::sink());
        }
    }
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn metadata_skips_large_payload_and_fake_usage_strings() {
        let fake = r#"{\"usage\":{\"total_tokens\":999999}}"#;
        let body = serde_json::json!({"data":[{"b64_json":"A".repeat(2*1024*1024)}],"choices":[{"text":fake}],"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20}}).to_string();
        let metadata = read_json(Cursor::new(body));
        assert_eq!(metadata.tokens.total.value, Some(20));
        assert_eq!(metadata.tokens.coverage(), (1, 0, 0));
    }

    #[test]
    fn metadata_preserves_nested_partial_and_rejects_overflow() {
        let metadata = read_json(Cursor::new(br#"{"response":{"usage":{"input_tokens":3,"output_tokens":-1,"total_tokens":18446744073709551615}}}"#));
        assert_eq!(metadata.tokens.input.value, Some(3));
        assert_eq!(metadata.tokens.output.value, None);
        assert_eq!(metadata.tokens.total.value, None);
    }

    #[test]
    fn metadata_distinguishes_null_and_actual_provider_errors_in_json_and_sse() {
        for (error, expected_failure) in [
            (serde_json::Value::Null, false),
            (
                serde_json::json!({"type": "server_error", "message": "upstream failed"}),
                true,
            ),
        ] {
            let response = serde_json::json!({
                "status": "completed", "error": error,
                "usage": {"input_tokens": 12, "output_tokens": 8, "total_tokens": 20}
            });
            let json = read_json(Cursor::new(response.to_string()));
            let event = serde_json::json!({"type": "response.completed", "response": response});
            let sse = read_sse(io::BufReader::new(Cursor::new(format!(
                "data: {event}\n\ndata: [DONE]\n\n"
            ))));
            for metadata in [json, sse] {
                assert_eq!(metadata.failed, expected_failure);
                assert_eq!(metadata.tokens.total.value, Some(20));
                assert_eq!(metadata.tokens.coverage(), (1, 0, 0));
            }
        }
    }

    #[test]
    fn metadata_scalar_noise_does_not_erase_valid_usage() {
        let metadata = read_json(Cursor::new(br#"{"message":"ordinary text","response":123,"type":false,"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20}}"#));
        assert_eq!(metadata.tokens.total.value, Some(20));
        assert!(!metadata.failed);
    }

    #[test]
    fn metadata_truncated_payload_retains_completed_fields_and_missing_suffix() {
        for (payload, output, total) in [
            (
                r#"{"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20},"data":[{"b64_json":"AAA"#,
                Some(8),
                Some(20),
            ),
            (
                r#"{"message":{"usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20},"content":[{"text":"AAA"#,
                Some(8),
                Some(20),
            ),
            (
                r#"{"response":{"usage":{"input_tokens":12},"output":[{"text":"AAA"#,
                None,
                None,
            ),
        ] {
            let metadata = read_json(Cursor::new(payload));
            assert_eq!(metadata.tokens.input.value, Some(12));
            assert_eq!(metadata.tokens.input.source, TokenSource::Provider);
            assert_eq!(metadata.tokens.output.value, output);
            assert_eq!(metadata.tokens.total.value, total);
        }
    }

    #[test]
    fn metadata_does_not_promote_an_uncompleted_usage_object() {
        for payload in [
            r#"{"usage":{"input_tokens":12"#,
            r#"{"usage":{"input_tokens":12,"output_tokens":8e"#,
        ] {
            let metadata = read_json(Cursor::new(payload));
            assert_eq!(metadata.tokens.input.value, None);
            assert_eq!(metadata.tokens.output.value, None);
            assert_eq!(metadata.tokens.total.value, None);
        }
    }

    #[test]
    fn metadata_sse_skips_large_events_and_merges_native_usage() {
        let delta=serde_json::json!({"type":"content_block_delta","delta":{"text":"X".repeat(2*1024*1024)}}).to_string();
        let body=format!("event: message_start\ndata: {{\"message\":{{\"usage\":{{\"input_tokens\":4}}}}}}\n\ndata: {delta}\n\ndata: {{\"usage\":{{\"output_tokens\":6,\"total_tokens\":10}}}}\n\ndata: [DONE]\n\n");
        let metadata = read_sse(io::BufReader::new(Cursor::new(body)));
        assert_eq!(metadata.tokens.input.value, Some(4));
        assert_eq!(metadata.tokens.total.value, Some(10));
    }
}
