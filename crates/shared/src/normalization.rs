//! Source-independent normalization contracts.

use std::{error::Error, fmt};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Region, Source};

/// A source document supplied to a source normalizer.
pub struct SourceDocument<'a> {
    /// URL that produced the document.
    pub source_url: &'a str,
    /// HTTP media type reported by the source.
    pub content_type: &'a str,
    /// Raw response bytes.
    pub body: &'a [u8],
}

/// A normalized record before persistence assigns an entry identifier.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NormalizedPolicyRecord {
    /// Stable identifier supplied by the publishing source.
    pub source_external_id: String,
    /// Human-readable policy title.
    pub title: String,
    /// Geographic jurisdiction of the source.
    pub region: Region,
    /// Government organization responsible for the record.
    pub agency: String,
    /// Publication or effective date, when the source supplies one.
    pub publication_date: Option<NaiveDate>,
    /// Current source-provided policy status.
    pub status: String,
    /// Canonical link back to the source material.
    pub source_url: String,
    /// Deterministic source fields used for change detection.
    pub canonical_content: Value,
}

/// Common interface implemented by category- and region-specific normalizers.
pub trait PolicyNormalizer: Send + Sync {
    /// Returns whether this normalizer can process the configured source.
    fn supports(&self, source: &Source) -> bool;

    /// Parses a fetched source document into normalized records.
    fn normalize(
        &self,
        source: &Source,
        document: SourceDocument<'_>,
    ) -> Result<Vec<NormalizedPolicyRecord>, NormalizationError>;
}

/// A source document could not be parsed into a policy record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizationError {
    message: String,
}

impl NormalizationError {
    /// Creates a normalization error suitable for a per-source failure report.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for NormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for NormalizationError {}
