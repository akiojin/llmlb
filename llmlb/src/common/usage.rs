//! Immutable admission identity and token measurement for organization usage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identity captured from authenticated server state at HTTP admission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageAttribution {
    /// Server-generated logical request identifier.
    pub request_id: Uuid,
    /// UTC admission time, retained across streaming and retries.
    pub received_at: DateTime<Utc>,
    /// Organization at admission; absent for unassigned legacy credentials.
    pub organization_id: Option<Uuid>,
    /// Usage account, independent of the API key issuer.
    pub account_id: Option<Uuid>,
    /// Credential identifier, when an API key authenticated the request.
    pub api_key_id: Option<Uuid>,
    /// Authentication source, such as `api_key`, `jwt`, or `legacy`.
    pub auth_source: String,
}

/// Provenance of one token field. Missing data is never measured zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
    /// No usable observation exists.
    #[default]
    Missing,
    /// Existing tokenizer estimated the count.
    Estimated,
    /// An upstream provider reported a valid integer.
    Provider,
}

impl TokenSource {
    /// Database representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Estimated => "estimated",
            Self::Provider => "provider",
        }
    }

    /// Parse stored provenance, failing closed for unknown historical values.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "provider" => Self::Provider,
            "estimated" => Self::Estimated,
            _ => Self::Missing,
        }
    }
}

/// One optional count with its source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenMeasurement {
    /// Count; absent means missing.
    pub value: Option<u32>,
    /// Source of the count.
    pub source: TokenSource,
}

impl TokenMeasurement {
    /// Construct an observation, ensuring absent values stay missing.
    pub fn new(value: Option<u32>, source: TokenSource) -> Self {
        Self {
            value,
            source: if value.is_some() {
                source
            } else {
                TokenSource::Missing
            },
        }
    }

    /// Retain provider observations over estimates and missing updates.
    pub fn merge(self, incoming: Self) -> Self {
        let rank = |source| match source {
            TokenSource::Missing => 0,
            TokenSource::Estimated => 1,
            TokenSource::Provider => 2,
        };
        if incoming.value.is_some() && rank(incoming.source) >= rank(self.source) {
            incoming
        } else {
            self
        }
    }
}

/// Token fields and coverage for one logical HTTP request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageTokens {
    /// Input token observation.
    pub input: TokenMeasurement,
    /// Output token observation.
    pub output: TokenMeasurement,
    /// Total token observation; partial reports stay partial.
    pub total: TokenMeasurement,
}

impl UsageTokens {
    /// Convert a legacy token value with an explicit source.
    pub fn from_usage(usage: &crate::token::TokenUsage, source: TokenSource) -> Self {
        Self {
            input: TokenMeasurement::new(usage.input_tokens, source),
            output: TokenMeasurement::new(usage.output_tokens, source),
            total: TokenMeasurement::new(usage.total_tokens, source),
        }
    }

    /// Merge each field independently, preferring reported values.
    pub fn merge(&self, incoming: &Self) -> Self {
        Self {
            input: self.input.merge(incoming.input),
            output: self.output.merge(incoming.output),
            total: self.total.merge(incoming.total),
        }
    }

    /// Mutually exclusive measured, estimated, and missing request counters.
    pub fn coverage(&self) -> (i64, i64, i64) {
        let fields = [self.input, self.output, self.total];
        if fields
            .iter()
            .any(|field| field.value.is_none() || field.source == TokenSource::Missing)
        {
            (0, 0, 1)
        } else if fields
            .iter()
            .any(|field| field.source == TokenSource::Estimated)
        {
            (0, 1, 0)
        } else {
            (1, 0, 0)
        }
    }
}
