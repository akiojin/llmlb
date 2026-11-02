//! Immutable inference attribution and token coverage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Attribution fixed by the server after authorization and input validation.
///
/// The issuer of an API key is deliberately absent: it is audit metadata, not
/// the account whose inference usage is being measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageAttribution {
    /// Server-generated identity for one accepted HTTP inference request.
    pub request_id: Uuid,
    /// Usage account at acceptance; never resolved again when the body ends.
    pub account_id: Option<Uuid>,
    /// Organization at acceptance; absent for legacy unattributed requests.
    pub organization_id: Option<Uuid>,
    /// UTC acceptance time, also the authority for the daily/monthly bucket.
    pub received_at: DateTime<Utc>,
    /// Authenticating key, when the request did not use a JWT session.
    pub api_key_id: Option<Uuid>,
    /// Authentication mechanism, selected by the server.
    pub auth_source: String,
}

/// Provenance of the available token values, separate from their completeness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
    /// Nonnegative token values reported by the upstream provider.
    Provider,
    /// Values produced by the existing local estimator.
    Estimated,
    /// No usable token observation was available.
    #[default]
    Missing,
}

impl TokenSource {
    /// Stable storage and API representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Estimated => "estimated",
            Self::Missing => "missing",
        }
    }
}

/// Final token observation for one logical request.
///
/// A missing field remains `None`, including a provider's omitted total. A
/// measured zero is `Some(0)`. Partial provider usage is not completed using an
/// estimate, so both its provenance and field-level coverage remain visible.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageTokens {
    /// Observed or estimated input tokens, if available.
    pub input_tokens: Option<u64>,
    /// Observed or estimated output tokens, if available.
    pub output_tokens: Option<u64>,
    /// Observed or estimated total; never inferred from partial provider data.
    pub total_tokens: Option<u64>,
    /// Provenance shared by the available values.
    pub source: TokenSource,
}

impl UsageTokens {
    /// A provider observation, which may be partial or an explicit zero.
    pub const fn provider(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens,
            source: TokenSource::Provider,
        }
    }

    /// An estimate; omitted values remain missing rather than becoming zero.
    pub const fn estimated(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens,
            source: TokenSource::Estimated,
        }
    }

    /// An interrupted or completed request with no usable token observation.
    pub const fn missing() -> Self {
        Self {
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            source: TokenSource::Missing,
        }
    }
}
