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

/// The reason a normalized record is unsuitable for policy/news persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordQualityRejection {
    /// The record is only a generic source heading with no substantive content.
    GenericHeadingOnlyContent,
    /// The title is navigation or other source boilerplate.
    GenericTitle,
    /// Neither an external identifier nor an item URL identifies the record.
    MissingStableIdentity,
}

impl fmt::Display for RecordQualityRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenericHeadingOnlyContent => formatter.write_str("generic heading-only content"),
            Self::GenericTitle => formatter.write_str("generic navigation or boilerplate title"),
            Self::MissingStableIdentity => formatter.write_str("missing stable item identity"),
        }
    }
}

/// Rejects source landing-page and navigation records before they reach persistence.
///
/// `document_url` is the fetched source feed, search result, or landing page. A
/// record must therefore have its own identifier or point to a different item URL.
pub fn validate_record_quality(
    record: &NormalizedPolicyRecord,
    document_url: &str,
) -> Result<(), RecordQualityRejection> {
    let normalized_title = normalize_text(&record.title);

    if is_generic_title(&normalized_title) {
        if has_only_generic_heading_content(record) {
            return Err(RecordQualityRejection::GenericHeadingOnlyContent);
        }

        return Err(RecordQualityRejection::GenericTitle);
    }

    if !has_stable_identity(record, document_url) {
        return Err(RecordQualityRejection::MissingStableIdentity);
    }

    Ok(())
}

fn is_generic_title(title: &str) -> bool {
    matches!(
        title,
        "language selection"
            | "request access"
            | "artificial intelligence"
            | "news and updates"
            | "search"
    )
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn has_only_generic_heading_content(record: &NormalizedPolicyRecord) -> bool {
    let Value::Object(fields) = &record.canonical_content else {
        return false;
    };

    fields.iter().all(|(key, value)| {
        matches!(
            key.as_str(),
            "title" | "agency" | "source_name" | "publication_date" | "status" | "source_url"
        ) || (key == "description" && value_is_empty(value))
    })
}

fn value_is_empty(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty())
}

fn has_stable_identity(record: &NormalizedPolicyRecord, document_url: &str) -> bool {
    let external_id = record.source_external_id.trim();
    let has_external_id = !external_id.is_empty() && !same_resource(external_id, document_url);
    let item_url = record.source_url.trim();
    let has_item_url = !item_url.is_empty() && !same_resource(item_url, document_url);

    has_external_id || has_item_url
}

fn same_resource(left: &str, right: &str) -> bool {
    left.trim_end_matches('/') == right.trim_end_matches('/')
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

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::Region;

    use super::{validate_record_quality, NormalizedPolicyRecord, RecordQualityRejection};

    fn record(
        title: &str,
        external_id: &str,
        source_url: &str,
        canonical_content: Value,
    ) -> NormalizedPolicyRecord {
        NormalizedPolicyRecord {
            source_external_id: external_id.to_owned(),
            title: title.to_owned(),
            region: Region::UnitedStates,
            agency: "Example agency".to_owned(),
            publication_date: None,
            status: "published".to_owned(),
            source_url: source_url.to_owned(),
            canonical_content,
        }
    }

    #[test]
    fn rejects_generic_navigation_titles_and_bare_headings() {
        let document_url = "https://example.test/feed";
        let heading = record(
            " Artificial   intelligence ",
            document_url,
            document_url,
            json!({ "title": "Artificial intelligence", "source_url": document_url }),
        );
        let navigation = record(
            "Request Access",
            "article-1",
            "https://example.test/articles/1",
            json!({ "title": "Request Access", "description": "Navigation" }),
        );

        assert_eq!(
            validate_record_quality(&heading, document_url),
            Err(RecordQualityRejection::GenericHeadingOnlyContent)
        );
        assert_eq!(
            validate_record_quality(&navigation, document_url),
            Err(RecordQualityRejection::GenericTitle)
        );
    }

    #[test]
    fn rejects_landing_page_fallback_identity() {
        let document_url = "https://example.test/search";
        let landing_page = record(
            "A real-looking title",
            document_url,
            document_url,
            json!({ "title": "A real-looking title" }),
        );

        assert_eq!(
            validate_record_quality(&landing_page, document_url),
            Err(RecordQualityRejection::MissingStableIdentity)
        );
    }

    #[test]
    fn accepts_article_url_or_source_identifier() {
        let document_url = "https://example.test/feed";
        let item_url = record(
            "AI standards update",
            document_url,
            "https://example.test/articles/ai-standards",
            json!({ "title": "AI standards update", "description": "Details" }),
        );
        let external_id = record(
            "Policy update",
            "DOC-42",
            document_url,
            json!({ "title": "Policy update" }),
        );

        assert!(validate_record_quality(&item_url, document_url).is_ok());
        assert!(validate_record_quality(&external_id, document_url).is_ok());
    }
}
